//! Optional local web UI (`recalld ui`, feature `ui`).
//!
//! The browser talks plain HTTP to a loopback-only axum server in the
//! `recalld ui` process, which talks to the daemon over its Unix socket like
//! any other client. It never opens the store itself. See `docs/ui/SPEC.md`.

pub mod agent;
mod assets;
pub mod daemon;
pub mod events;
pub mod http;
mod memories;
mod security;
mod sysinfo;
pub mod wire;

#[cfg(test)]
mod tests;

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

pub use http::{DaemonPool, Starter, UiState};
pub use security::MUTATION_HEADER;

/// Daemon connections kept by one `recalld ui` process.
const POOL_SIZE: usize = 4;

pub struct UiOptions {
    /// `None` picks a free port.
    pub port: Option<u16>,
    pub open_browser: bool,
    pub socket: std::path::PathBuf,
    pub starter: Option<Starter>,
}

/// A bound, not yet serving UI server.
pub struct BoundUi {
    listener: tokio::net::TcpListener,
    state: UiState,
}

impl BoundUi {
    /// Bind `127.0.0.1` only; there is no option to bind anything else.
    pub async fn bind(options: &UiOptions) -> std::io::Result<Self> {
        let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, options.port.unwrap_or(0)));
        let listener = tokio::net::TcpListener::bind(addr).await?;
        let port = listener.local_addr()?.port();
        Ok(Self {
            listener,
            state: UiState {
                token: security::new_token().into(),
                port,
                daemon: Arc::new(DaemonPool::new(options.socket.clone(), POOL_SIZE)),
                starter: options.starter.clone(),
            },
        })
    }

    pub fn port(&self) -> u16 {
        self.state.port
    }

    pub fn token(&self) -> &str {
        &self.state.token
    }

    /// The one-time link that signs the browser in.
    pub fn url(&self) -> String {
        format!(
            "http://127.0.0.1:{}/?t={}",
            self.state.port, self.state.token
        )
    }

    pub async fn serve(self, shutdown: impl std::future::Future<Output = ()> + Send + 'static) {
        let app = http::router(self.state);
        if let Err(e) = axum::serve(self.listener, app)
            .with_graceful_shutdown(shutdown)
            .await
        {
            tracing::error!(%e, "UI server error");
        }
    }
}

/// `recalld ui`: bind, print and open the link, serve until Ctrl-C.
pub async fn run(options: UiOptions) -> std::io::Result<()> {
    let bound = BoundUi::bind(&options).await?;
    let url = bound.url();
    if !assets::available() {
        eprintln!(
            "warning: this binary has no built UI assets; run `make ui` and rebuild with --features ui"
        );
    }
    eprintln!("recalld ui {} at {url}", env!("CARGO_PKG_VERSION"));
    eprintln!("Press Ctrl-C to stop.");
    if options.open_browser {
        open_browser(&url);
    }
    bound
        .serve(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await;
    Ok(())
}

fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(not(target_os = "macos"))]
    let program = "xdg-open";
    if let Err(e) = std::process::Command::new(program)
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        eprintln!("could not open a browser ({e}); open the link above yourself");
    }
}
