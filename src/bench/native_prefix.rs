//! Private synthetic native-prefix consumer; ordinary startup remains separate.
use std::sync::Arc;

use crate::config::{EmbeddingProvider, RecalldConfig};
use crate::mcp::protocol::{
    ResourceInfo, ResourceReadResult, ResourceTemplate, ToolCallResult, ToolInfo,
};
use crate::mcp::server::McpHandler;
use crate::storage::{RedbStorageEngine, offline_snapshot::OfflineSnapshot};
use crate::time::{Clock, ManualClock};

use super::frozen::{FrozenControls, OwnedEvaluator};

pub(crate) const TRACK: &str = "isolated-native-prefix-checkpoint-v1";
pub(crate) const HASH_CONTRACT: &str = "native-logical-export-v1";

/// This named synthetic track supports only the actual configured OpenAI-shaped
/// loopback provider. None is an unknown revision, never an invented pin.
fn profile(config: &RecalldConfig) -> anyhow::Result<crate::model::schema::EmbeddingProfile> {
    let url = reqwest::Url::parse(&config.embedding.base_url)?;
    anyhow::ensure!(
        config.embedding.provider == EmbeddingProvider::OpenAI
            && url.scheme() == "http"
            && matches!(url.host_str(), Some("127.0.0.1" | "[::1]" | "::1"))
            && url.username().is_empty()
            && url.password().is_none(),
        "native prefix requires the explicit loopback synthetic provider"
    );
    let mut profile = crate::embedding::runtime::native_profile(
        crate::model::schema::ProviderKind::Openai,
        &config.embedding.model_name,
        config.embedding.dimensions,
    );
    profile.document_preprocessing.prefix = config.embedding.document_prefix.clone();
    profile.query_preprocessing.prefix = config.embedding.query_prefix.clone();
    profile.validate().map_err(anyhow::Error::msg)?;
    Ok(profile)
}

pub(super) async fn open_core(
    config: RecalldConfig,
    clock: Arc<dyn Clock>,
    ids: Box<dyn crate::storage::ids::IdSource>,
) -> anyhow::Result<crate::Recalld> {
    anyhow::ensure!(
        config.decay.disable_sweep && !config.rif.enabled && !config.graph.autolink_enabled,
        "native prefix requires sweep, RIF and autolink disabled"
    );
    let profile = profile(&config)?;
    let root = std::path::Path::new(&config.storage.data_dir);
    let snapshot = OfflineSnapshot::open(root)?;
    anyhow::ensure!(
        snapshot.control().control_schema_version == 2,
        "native prefix requires schema2"
    );
    anyhow::ensure!(
        snapshot.namespaces().iter().any(|ns| ns.name == "default"),
        "native prefix requires an existing default namespace"
    );
    for ns in snapshot.namespaces() {
        crate::embedding::profile::require_compatible(ns, &profile)?;
    }
    let generation = snapshot.control().generation_ref();
    drop(snapshot);
    // Both strict preflight and the actual opener retain real owner locks. The
    // fresh opener revalidates history and must not change the admitted prefix.
    let engine = RedbStorageEngine::open_with_ids(root, true, ids)?;
    anyhow::ensure!(
        engine.control_state()?.generation_ref() == generation,
        "native prefix changed during owner admission"
    );
    Ok(crate::Recalld::with_owned_storage(config, clock, engine).await?)
}

impl OwnedEvaluator {
    /// Existing, strictly validated private prefix. No migration, genesis or
    /// ordinary native selector is implied by this fixture constructor.
    pub async fn from_native_prefix(config: RecalldConfig) -> anyhow::Result<Self> {
        let clock = Arc::new(ManualClock::new(1_767_225_600_000));
        let mut seed = [0; 32];
        seed[..8].copy_from_slice(&7_u64.to_be_bytes());
        let core = Arc::new(
            open_core(
                config,
                clock.clone(),
                Box::new(crate::storage::ids::SeededIds::new(seed)),
            )
            .await?,
        );
        let controls = FrozenControls::new(core.derived_owner(), clock);
        Ok(Self {
            core,
            controls,
            fixture_ancestors: Vec::new(),
            native_prefix: true,
        })
    }

    pub fn is_native_prefix(&self) -> bool {
        self.native_prefix
    }

    /// Restrict the actor before dispatch to any canonical owner/provider. The
    /// allowed requests still use the real public tools and production bridge.
    pub fn actor_handler(&self) -> Arc<dyn McpHandler> {
        let bridge = Arc::new(crate::mcp::bridge_adapters::direct_bridge(
            &self.core,
            "default".into(),
        ));
        if self.native_prefix {
            Arc::new(FalseReadActor(bridge))
        } else {
            bridge
        }
    }
}

struct FalseReadActor(Arc<dyn McpHandler>);
#[async_trait::async_trait]
impl McpHandler for FalseReadActor {
    fn tools(&self) -> Vec<ToolInfo> {
        self.0
            .tools()
            .into_iter()
            .filter(|t| matches!(t.name.as_str(), "get_memory" | "recall_memories"))
            .map(|mut t| {
                t.description
                    .push_str(" Private native-prefix track: explicit recordAccess=false only.");
                t.input_schema["properties"]["recordAccess"] = serde_json::json!({"const":false});
                if let Some(required) = t.input_schema["required"].as_array_mut() {
                    if !required.iter().any(|v| v == "recordAccess") {
                        required.push("recordAccess".into());
                    }
                }
                t
            })
            .collect()
    }
    fn resources(&self) -> Vec<ResourceInfo> {
        Vec::new()
    }
    fn resource_templates(&self) -> Vec<ResourceTemplate> {
        Vec::new()
    }
    async fn call_tool(&self, name: &str, arguments: serde_json::Value) -> ToolCallResult {
        if !matches!(name, "get_memory" | "recall_memories")
            || arguments.get("recordAccess") != Some(&serde_json::Value::Bool(false))
            || arguments.get("contractVersion").is_some_and(|v| v != 1)
            || arguments.get("apiVersion").is_some()
        {
            return ToolCallResult::error(
                "UNSUPPORTED_NATIVE_PREFIX_ACTION: API1 false get/search only",
            );
        }
        self.0.call_tool(name, arguments).await
    }
    async fn read_resource(&self, _: &str) -> Result<ResourceReadResult, String> {
        Err("UNSUPPORTED_NATIVE_PREFIX_ACTION: use explicit false get/search".into())
    }
}
