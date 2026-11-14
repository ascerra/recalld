//! Bounded legacy online-write diagnostic, not a latency/throughput benchmark.
use anyhow::{Result, ensure};
use async_trait::async_trait;
use clap::Parser;
use recalld::{
    config::{EmbeddingProvider as ProviderKind, RecalldConfig},
    daemon::{client::DaemonClient, server::DaemonServer},
    embedding::{EmbeddingError, EmbeddingProvider},
    mcp::{bridge::*, bridge_adapters::*, protocol::ContentBlock, tools},
    model::{AccessKind, MemoryId, NamespaceConfig},
    search::VectorIndex,
    storage::{
        RedbStorageEngine, StorageEngine as CanonicalStorage, control::MutationContext,
        insertion::InsertMemory,
    },
    system::Recalld,
    time::ManualClock,
};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
const NOW: i64 = 1_767_225_600_000;
const DIM: usize = 384;
#[derive(Parser)]
struct Args {
    #[arg(long)]
    root: PathBuf,
    #[arg(long)]
    path: String,
    #[arg(long, default_value_t = 100)]
    records: usize,
    #[arg(long, default_value_t = 0)]
    history: usize,
    #[arg(long)]
    edges: bool,
}
fn vector() -> Vec<f32> {
    let mut v = vec![0.; DIM];
    v[0] = 1.;
    v
}
struct LocalProvider(AtomicUsize);
#[async_trait]
impl EmbeddingProvider for LocalProvider {
    async fn embed(&self, _: &str) -> Result<Vec<f32>, EmbeddingError> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Ok(vector())
    }
    fn dimensions(&self) -> usize {
        DIM
    }
    fn model_name(&self) -> &str {
        "diagnostic-deterministic-local"
    }
}
#[derive(Clone)]
struct Sink(Arc<Mutex<fs::File>>, Arc<AtomicUsize>);
impl Write for Sink {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        let result = self.0.lock().unwrap().write(b);
        if result.is_err() {
            self.1.fetch_add(1, Ordering::Relaxed);
        }
        result
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Sink {
    type Writer = Self;
    fn make_writer(&'a self) -> Self {
        self.clone()
    }
}
fn bridge(system: &Recalld, provider: Arc<dyn EmbeddingProvider>) -> McpBridge {
    let tz = recalld::time::resolve_timezone(&system.config().timezone);
    McpBridge {
        search: Arc::new(McpSearchAdapter::new(
            system.query_engine().clone(),
            provider.clone(),
            system.storage().clone(),
            system.graph().clone(),
            tz,
        )),
        storage: Arc::new(
            McpStorageAdapter::new(
                system.storage().clone(),
                system.cache().clone(),
                provider,
                system.vector_index().clone(),
                system.fts_index().clone(),
                system.entity_index().clone(),
                system.graph().clone(),
                Arc::new(system.config().clone()),
                tz,
            )
            .with_clock(system.clock().clone()),
        ),
        namespaces: Arc::new(McpNamespaceAdapter::new(
            system.storage().clone(),
            system.cache().clone(),
            system.vector_index().clone(),
            system.fts_index().clone(),
            system.entity_index().clone(),
            system.graph().clone(),
            tz,
        )),
        health: Arc::new(McpHealthAdapter::new(system.storage().clone())),
        default_namespace: "fixture".into(),
        timezone: tz,
    }
}
fn unix_ns() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}
#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<()> {
    let args = Args::parse();
    ensure!(
        std::env::var("RECALLD_DIAGNOSTIC_ISOLATED").as_deref() == Ok("1"),
        "isolated diagnostic opt-in required"
    );
    ensure!(
        args.root == PathBuf::from("/study"),
        "only the isolated /study mount is supported"
    );
    ensure!(
        ["adapter-explicit", "mcp-local-provider", "daemon-explicit"].contains(&args.path.as_str()),
        "unknown path"
    );
    ensure!(
        [100, 1000].contains(&args.records) && [0, 1000].contains(&args.history),
        "outside bounded diagnostic cells"
    );
    ensure!(
        std::env::var_os("RECALLD_DAEMON_SOCKET")
            == Some(args.root.join("daemon.sock").into_os_string()),
        "isolated daemon socket required"
    );
    let store = args.root.join("store");
    ensure!(!store.exists(), "refuse to reuse a diagnostic store");
    let mut storage = RedbStorageEngine::open(&store)?;
    let mut ns = NamespaceConfig::default_namespace(NOW);
    ns.name = "fixture".into();
    ns.embedding_dim = DIM as u32;
    let ns = storage.create_namespace(&ns)?;
    let root_id = MemoryId::from_uuid(uuid::Uuid::from_u128(1));
    for n in 0..args.records {
        storage.insert_memory_with_receipt(
            &MutationContext::unkeyed(ns),
            &InsertMemory {
                id: Some(MemoryId::from_uuid(uuid::Uuid::from_u128(n as u128 + 1))),
                summary: format!("seed {n}"),
                full_text: Some("synthetic seed payload".into()),
                tags: vec![],
                initial_stability: Some(1.),
                created_at: Some(NOW),
                embedding: Some(vector()),
                parent_id: (args.edges && n > 0).then_some(root_id),
                supersedes: None,
            },
            &[],
            NOW,
        )?;
    }
    drop(storage);
    let mut config = RecalldConfig::default();
    config.storage.data_dir = store.to_str().unwrap().into();
    config.embedding.provider = ProviderKind::Passthrough;
    config.embedding.dimensions = DIM;
    config.decay.disable_sweep = true;
    config.graph.autolink_enabled = false;
    config.rif.enabled = false;
    let system = Recalld::with_clock(config, Arc::new(ManualClock::new(NOW))).await?;
    system.complete_pending_effects().await?;
    let mut history_receipts = fs::File::create(args.root.join("history-receipts.jsonl"))?;
    for _ in 0..args.history {
        let receipts = recalld::decay::observation::record_exposures(
            system.storage().clone(),
            system.cache().clone(),
            vec![(root_id, AccessKind::DirectRetrieval)],
            NOW,
        )
        .await?;
        ensure!(
            receipts.len() == 1,
            "history setup requires one real receipt per event"
        );
        for receipt in receipts {
            writeln!(history_receipts, "{}", serde_json::to_string(&receipt)?)?;
        }
    }
    let canonical = system.storage().clone();
    let vectors = system.vector_index().clone();
    let graph = system.graph().clone();
    let fts = system.fts_index().clone();
    let entities = system.entity_index().clone();
    let provider = Arc::new(LocalProvider(AtomicUsize::new(0)));
    let direct = bridge(&system, provider.clone());
    let before = {
        let s = canonical.read().unwrap();
        json!({"control":s.control_state()?,"records":s.scan_all()?.len(),"edges":s.load_all_edges()?.len()})
    };
    fs::write(
        args.root.join("setup.json"),
        serde_json::to_vec_pretty(
            &json!({"path":args.path,"records":args.records,"legacy_observation_receipts":args.history,"edges_requested":args.edges,"dimensions":DIM,"clock_ms":NOW,"before":before}),
        )?,
    )?;
    let (client, server) = if args.path == "daemon-explicit" {
        let socket = args.root.join("daemon.sock");
        let server_socket = socket.clone();
        let server = tokio::spawn(async move {
            DaemonServer::new(system)
                .run(&server_socket, Duration::from_secs(120))
                .await
        });
        // Startup only, outside all write windows. Do not retry any mutation.
        tokio::time::timeout(Duration::from_secs(5), async {
            while !socket.exists() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await?;
        let client = DaemonClient::connect(&socket).await?;
        client.ping().await?;
        (Some(client), Some(server))
    } else {
        (None, None)
    };
    let mut sink = Sink(
        Arc::new(Mutex::new(fs::File::create(
            args.root.join("events.jsonl"),
        )?)),
        Arc::new(AtomicUsize::new(0)),
    );
    tracing_subscriber::fmt()
        .json()
        .with_ansi(false)
        .with_thread_ids(true)
        .with_env_filter("recalld::performance=trace")
        .with_writer(sink.clone())
        .init();
    let mut writes = fs::File::create(args.root.join("writes.jsonl"))?;
    for n in 0..4 {
        let token = format!("onlineproof{n}");
        let input = StoreInput {
            summary: token.clone(),
            full_text: Some("synthetic online payload".into()),
            tags: vec![],
            entities: vec![token.clone()],
            topics: vec![],
            emotions: vec![],
            namespace: "fixture".into(),
            embedding: Some(vector()),
            initial_stability: Some(1.),
            parent_id: None,
            supersedes: None,
            check_duplicates: Some(false),
        };
        let control_before = canonical.read().unwrap().control_state()?;
        let calls_before = provider.0.load(Ordering::Relaxed);
        let begin_ns = unix_ns();
        let start = Instant::now();
        let response: Result<Value> = match args.path.as_str() {
            "adapter-explicit" => direct
                .storage
                .store_memory(input)
                .await
                .map(|r| serde_json::to_value(r).unwrap())
                .map_err(Into::into),
            "mcp-local-provider" => {
                // The actual tool has no embedding wire field. No schema extension.
                let result = tools::dispatch_tool(&direct, "store_memory", json!({"summary":token,"fullText":"synthetic online payload","entities":[token],"namespace":"fixture","checkDuplicates":false})).await;
                if result.is_error == Some(true) {
                    Err(anyhow::anyhow!(serde_json::to_string(&result)?))
                } else {
                    let ContentBlock::Text { text } = &result.content[0];
                    serde_json::from_str(text).map_err(Into::into)
                }
            }
            "daemon-explicit" => client
                .as_ref()
                .unwrap()
                .call("store_memory", serde_json::to_value(input)?)
                .await
                .map_err(Into::into),
            _ => unreachable!(),
        };
        let ack_ns = unix_ns();
        let elapsed_ns = start.elapsed().as_nanos();
        sink.flush()?;
        let end_ns = unix_ns();
        let provider_calls = provider.0.load(Ordering::Relaxed) - calls_before;
        writeln!(
            writes,
            "{}",
            json!({"index":n,"warmup":n==0,"begin_ns":begin_ns.to_string(),"ack_ns":ack_ns.to_string(),"end_ns":end_ns.to_string(),"elapsed_ns":elapsed_ns,"with_sink_flush_ns":start.elapsed().as_nanos(),"provider_calls":provider_calls,"response":response.as_ref().ok(),"error":response.as_ref().err().map(ToString::to_string)})
        )?;
        writes.flush()?;
        let response = response?;
        let id = MemoryId::from_uuid(
            response["id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("missing stored ID: {response}"))?
                .parse()?,
        );
        let control_after = canonical.read().unwrap().control_state()?;
        ensure!(
            control_after.committed_generation.get()
                == control_before.committed_generation.get() + 1,
            "unexpected committed generation delta"
        );
        ensure!(
            canonical.read().unwrap().get_record(id)?.is_some(),
            "missing canonical record"
        );
        ensure!(
            vectors.read().await.get_vector(id).is_some(),
            "vector not published"
        );
        ensure!(graph.read().await.contains(&id), "graph not published");
        ensure!(
            fts.lock()
                .await
                .search(ns, &token, 10)?
                .iter()
                .any(|r| r.0 == id),
            "FTS not published"
        );
        ensure!(
            entities
                .read()
                .await
                .find_by_entities(&[token], root_id)
                .iter()
                .any(|r| r.0 == id),
            "entities not published"
        );
        ensure!(
            direct
                .storage
                .get_memory(id, false, Some("fixture"))
                .await?
                .is_some(),
            "canonical admission not ready"
        );
        ensure!(
            provider_calls == usize::from(args.path == "mcp-local-provider"),
            "unexpected provider calls"
        );
    }
    if let Some(client) = client {
        client.call("shutdown", json!({})).await?;
        drop(client);
    }
    if let Some(server) = server {
        tokio::time::timeout(Duration::from_secs(5), server).await???;
    }
    sink.flush()?;
    ensure!(sink.1.load(Ordering::Relaxed) == 0, "trace sink I/O failed");
    fs::write(
        args.root.join("process-status.txt"),
        fs::read("/proc/self/status")?,
    )?;
    let s = canonical.read().unwrap();
    ensure!(
        s.scan_all()?.len() == args.records + 4,
        "final record count"
    );
    fs::write(
        args.root.join("complete.json"),
        serde_json::to_vec_pretty(
            &json!({"writes":4,"measured":3,"errors":0,"sink_errors":0,"control":s.control_state()?,"records":s.scan_all()?.len(),"edges":s.load_all_edges()?.len()}),
        )?,
    )?;
    Ok(())
}
