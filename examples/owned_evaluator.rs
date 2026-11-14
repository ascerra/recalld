//! Isolated legacy-v1 MCP actor with same-core evaluator-only inherited pipes.
//! Supports bounded fresh-owner fixture replay; no public v2 or full A6 claim.
use clap::Parser;
use recalld::config::{EmbeddingProvider, RecalldConfig};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[derive(Parser)]
struct Args {
    #[arg(long)]
    config: PathBuf,
    #[arg(long, conflicts_with = "restore_checkpoint")]
    native_prefix: bool,
    #[arg(long)]
    data_root: PathBuf,
    #[arg(long)]
    control_read_fd: i32,
    #[arg(long)]
    control_write_fd: i32,
    #[arg(long, requires = "checkpoint_sha256")]
    restore_checkpoint: Option<PathBuf>,
    #[arg(long, requires = "restore_checkpoint")]
    checkpoint_sha256: Option<String>,
}

#[cfg(unix)]
fn control_pipe(fd: i32, write: bool) -> anyhow::Result<std::fs::File> {
    use std::os::fd::FromRawFd;
    use std::os::unix::fs::FileTypeExt;
    anyhow::ensure!(
        fd >= 3,
        "control descriptors must be separate from actor stdio"
    );
    // Called before starting runtime threads. The supervisor transfers these
    // distinct inherited descriptors exclusively to this launcher.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    anyhow::ensure!(flags >= 0, "control descriptor is not open");
    let expected = if write {
        libc::O_WRONLY
    } else {
        libc::O_RDONLY
    };
    anyhow::ensure!(
        flags & libc::O_ACCMODE == expected,
        "control pipe direction mismatch"
    );
    let file = unsafe { std::fs::File::from_raw_fd(fd) };
    anyhow::ensure!(
        file.metadata()?.file_type().is_fifo(),
        "control descriptors must be pipes"
    );
    Ok(file)
}

#[cfg(unix)]
async fn run(args: Args, input: std::fs::File, output: std::fs::File) -> anyhow::Result<()> {
    // Explicit config only: no HOME config discovery, daemon fallback or provider
    // credentials. The supervisor separately allowlists the process environment.
    let mut config: RecalldConfig = toml::from_str(&std::fs::read_to_string(&args.config)?)?;
    let url = reqwest::Url::parse(&config.embedding.base_url)?;
    anyhow::ensure!(
        config.embedding.provider == EmbeddingProvider::OpenAI
            && url.scheme() == "http"
            && matches!(url.host_str(), Some("127.0.0.1" | "[::1]" | "::1"))
            && url.username().is_empty()
            && url.password().is_none(),
        "only an explicit loopback offline embedding fixture is allowed"
    );
    anyhow::ensure!(
        config.embedding.api_key_env == "RECALLD_EVAL_OFFLINE_KEY"
            && std::env::var("RECALLD_EVAL_OFFLINE_KEY").as_deref() == Ok("offline-fixture"),
        "an explicit synthetic fixture key is required"
    );
    anyhow::ensure!(
        args.native_prefix || !args.data_root.exists(),
        "data root must be a fresh destination"
    );
    if args.restore_checkpoint.is_none() && !args.native_prefix {
        std::fs::create_dir(&args.data_root)?;
    }
    use std::os::unix::fs::PermissionsExt;
    if args.restore_checkpoint.is_none() && !args.native_prefix {
        std::fs::set_permissions(&args.data_root, std::fs::Permissions::from_mode(0o700))?;
    }
    config.storage.data_dir = std::path::absolute(&args.data_root)?
        .to_string_lossy()
        .into_owned();
    config.decay.disable_sweep = true;
    config
        .validate()
        .map_err(|errors| anyhow::anyhow!("invalid explicit fixture config: {errors:?}"))?;
    let evaluator = Arc::new(if let Some(manifest) = args.restore_checkpoint {
        recalld::bench::frozen::OwnedEvaluator::from_checkpoint(
            config,
            &manifest,
            args.checkpoint_sha256.as_deref().unwrap(),
        )
        .await?
    } else if args.native_prefix {
        recalld::bench::frozen::OwnedEvaluator::from_native_prefix(config).await?
    } else {
        recalld::bench::frozen::OwnedEvaluator::new(config).await?
    });
    let server = Arc::new(tokio::sync::Mutex::new(recalld::mcp::McpServer::new(
        evaluator.actor_handler(),
    )));
    // EOF/error on either channel ends this child. It is never a successful
    // snapshot or a certificate about a request whose ACK was not received.
    tokio::select! {
        result = recalld::mcp::run_stdio(server) => result.map_err(Into::into),
        result = recalld::bench::controls::serve_owned(evaluator, tokio::fs::File::from_std(input), tokio::fs::File::from_std(output)) => result.map_err(Into::into),
    }
}

#[cfg(unix)]
fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    anyhow::ensure!(
        args.control_read_fd != args.control_write_fd,
        "control descriptors must differ"
    );
    let input = control_pipe(args.control_read_fd, false)?;
    let output = control_pipe(args.control_write_fd, true)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let result = runtime.block_on(run(args, input, output));
    // stdin/file reads use blocking workers. Their cancellation cannot indefinitely
    // hold this diagnostic child alive after the supervisor closes a channel.
    runtime.shutdown_timeout(Duration::from_secs(1));
    result
}
#[cfg(not(unix))]
fn main() {
    eprintln!("owned evaluator requires inherited Unix pipes");
    std::process::exit(2);
}
