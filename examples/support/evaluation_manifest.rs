//! Consumed only by the internal offline diagnostic, never a production API.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

pub const TRACK: &str = "manifest-internal-diagnostic-v2";
pub const TIME: i64 = 1_767_225_600_000;

pub fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Plan {
    pub records: usize,
    pub dimensions: usize,
    pub k: usize,
    pub warmup: usize,
    pub samples: usize,
    pub seed: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Artifact {
    pub file: String,
    pub sha256: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub manifest_version: u32,
    pub source_mapping_version: u32,
    pub trace_schema_version: u32,
    pub track: String,
    pub source_commit: String,
    pub source_tree_sha256: String,
    pub dirty_patch_sha256: String,
    pub source_files_sha256: BTreeMap<String, String>,
    pub lockfile_sha256: String,
    pub container_image_id: String,
    pub limits: Value,
    pub versions: Value,
    pub capabilities: Value,
    pub dataset: Value,
    pub embedding: Value,
    pub scoring: Value,
    pub execution: Plan,
    pub actor_inputs: Artifact,
    pub evaluator_labels: Artifact,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Source {
    pub source_id: String,
    pub group_id: String,
    pub conversation_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub role: String,
    pub source_timestamp: i64,
    pub ordinal: usize,
    pub summary: String,
    pub content_sha256: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ActorInputs {
    adapter_version: u32,
    sources: Vec<Source>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EvaluatorLabels {
    adapter_version: u32,
    annotation_unit: String,
    relevant: BTreeMap<String, Vec<String>>,
    // Deliberately never available through an actor-facing accessor.
    label_canary: String,
}

pub struct Validated {
    pub manifest: Manifest,
    pub environment_hash: String,
    pub sources: Vec<Source>,
    evaluator: EvaluatorLabels,
}

pub fn versions() -> Value {
    json!({"contractVersion":1,"retrievalSemanticsVersion":1,"rankingProfile":"baseline-v1",
        "lifecycleVersion":"legacy_access_v1","rifMode":"off","canonicalSchemaVersion":null,"canonicalRepresentation":"legacy-schema-none","controlSchemaVersion":1,
        "recordSchemaVersion":1,"backupFormatVersion":null,"exportFormatVersion":null,
        "offlineFormats":"not-used-or-advertised-by-this-track",
        "receiptVersion":1,"requestHashVersion":1,
        "derivedIndexFormats":"sourceFilesSha256; no independent format version advertised"})
}

pub fn scoring() -> Value {
    json!({"profile":"synthetic-unique-source-v1","annotationUnit":"synthetic-unit",
        "deduplication":"first-source-occurrence-before-cutoff","relevance":"binary",
        "recallCutoffs":[5,10,20],"ndcgCutoff":10,"allRequired":"all-returned-unique-sources",
        "emptyLabels":"unscored","missingAnnotations":"unknown","aggregation":"measured-query-events-only"})
}

pub fn capabilities() -> Value {
    json!({"publicApi":false,"publicV2":false,"nativeV2":false,"fullA6":false,"coherentReadView":false,"barrier":false,
        "snapshot":false,"reset":false,"deterministicIds":false,"deterministicRng":false,
        "clock":"fixed-lifecycle-only","clockAdvance":false,"sweeps":false,
        "initialTimeMs":TIME,"recordAccess":false,"graphDepth":0})
}

pub fn embedding(dimensions: usize) -> Value {
    json!({"generator":"splitmix64-f32-l2-v1","dimensions":dimensions,"modelRevision":null,
        "promptSha256":null,"tokenizerRevision":null,"role":"actor-query",
        "cacheMiss":"error","externalCalls":false})
}

fn hexadecimal(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

fn artifact(root: &Path, entry: &Artifact, expected_name: &str) -> Result<Vec<u8>> {
    ensure!(
        entry.file == expected_name && hexadecimal(&entry.sha256, 64),
        "invalid artifact reference"
    );
    let path = root.join(&entry.file);
    ensure!(
        fs::symlink_metadata(&path)?.file_type().is_file(),
        "artifact must be a regular file"
    );
    let bytes = fs::read(path)?;
    ensure!(
        hash(&bytes) == entry.sha256,
        "artifact hash mismatch: {}",
        entry.file
    );
    Ok(bytes)
}

fn require_rust_pins(root: &Path, directory: &Path, pins: &BTreeMap<String, String>) -> Result<()> {
    if !directory.exists() {
        return Ok(());
    }
    for item in fs::read_dir(directory)? {
        let item = item?;
        let kind = item.file_type()?;
        ensure!(!kind.is_symlink(), "symlink in source tree");
        if kind.is_dir() {
            require_rust_pins(root, &item.path(), pins)?;
        } else if item.path().extension().is_some_and(|ext| ext == "rs") {
            let path = item.path();
            let name = path
                .strip_prefix(root)?
                .to_str()
                .context("non-UTF8 source input")?;
            ensure!(
                pins.contains_key(name),
                "omitted runtime source pin: {name}"
            );
        }
    }
    Ok(())
}

impl Validated {
    /// No output directory, store, provider or mutation exists during validation.
    pub fn load(path: &Path) -> Result<Self> {
        ensure!(
            fs::symlink_metadata(path)?.file_type().is_file(),
            "manifest must be a regular file"
        );
        let bytes = fs::read(path)?;
        let m: Manifest = serde_json::from_slice(&bytes).context("invalid manifest")?;
        ensure!(
            m.manifest_version == 1 && m.source_mapping_version == 1 && m.trace_schema_version == 1,
            "unsupported independent harness version"
        );
        ensure!(
            m.track == TRACK && m.versions == versions() && m.scoring == scoring(),
            "unsupported track/runtime versions"
        );
        ensure!(
            m.capabilities == capabilities(),
            "unsupported runtime capability/clock/RNG request"
        );
        ensure!(
            m.limits
                == json!({"cpus":2,"memoryBytes":4294967296u64,"cargoJobs":2,"tokioWorkers":2}),
            "unsupported resource limits"
        );
        ensure!(
            hexadecimal(&m.source_commit, 40)
                && hexadecimal(&m.dirty_patch_sha256, 64)
                && m.container_image_id
                    .strip_prefix("sha256:")
                    .is_some_and(|s| hexadecimal(s, 64)),
            "unresolved source/image identity"
        );
        ensure!(
            option_env!("RECALLD_A6_SOURCE_COMMIT") == Some(m.source_commit.as_str())
                && option_env!("RECALLD_A6_DIRTY_PATCH_SHA256")
                    == Some(m.dirty_patch_sha256.as_str())
                && option_env!("RECALLD_A6_IMAGE_ID") == Some(m.container_image_id.as_str()),
            "manifest differs from compiled supervisor build identity"
        );
        ensure!(
            hexadecimal(&m.source_tree_sha256, 64)
                && hash(&serde_json::to_vec(&m.source_files_sha256)?) == m.source_tree_sha256
                && option_env!("RECALLD_A6_SOURCE_TREE_SHA256")
                    == Some(m.source_tree_sha256.as_str()),
            "manifest source tree differs from compiled source pins"
        );
        let p = &m.execution;
        ensure!(
            (100..=100_000).contains(&p.records)
                && p.records.is_multiple_of(10)
                && (4..=8192).contains(&p.dimensions)
                && (1..=200).contains(&p.k)
                && p.k <= p.records / 10
                && (1..=100_000).contains(&p.samples)
                && p.warmup <= 100_000,
            "unsupported execution plan"
        );
        ensure!(
            m.embedding == embedding(p.dimensions),
            "unsupported embedding/cache profile"
        );
        ensure!(
            m.dataset
                == json!({"kind":"generated-synthetic","upstreamUrl":null,
            "upstreamRevision":null,"thirdPartyLicense":null,"intendedUse":"contract-diagnostic-only",
            "adapterVersion":1,"groupSplit":"one-synthetic-regression-group; no training",
            "datasetRevision":m.actor_inputs.sha256}),
            "unresolved/external dataset, license or split"
        );
        let source_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        for directory in ["src", "tests", "examples", "benches"] {
            require_rust_pins(
                source_root,
                &source_root.join(directory),
                &m.source_files_sha256,
            )?;
        }
        for required in [
            "Cargo.toml",
            "rust-toolchain.toml",
            "Cargo.lock",
            "examples/offline_eval.rs",
            "examples/support/evaluation_manifest.rs",
            "scripts/offline-evaluation.py",
        ] {
            ensure!(
                m.source_files_sha256.contains_key(required),
                "missing required source pin: {required}"
            );
        }
        ensure!(
            m.source_files_sha256.get("Cargo.lock") == Some(&m.lockfile_sha256),
            "lockfile pin mismatch"
        );
        for (name, expected) in &m.source_files_sha256 {
            ensure!(
                !name.is_empty()
                    && Path::new(name)
                        .components()
                        .all(|p| matches!(p, std::path::Component::Normal(_)))
                    && hexadecimal(expected, 64),
                "invalid source pin"
            );
            let mut current = source_root.to_path_buf();
            for part in Path::new(name) {
                current.push(part);
                ensure!(
                    !fs::symlink_metadata(&current)?.file_type().is_symlink(),
                    "symlink source input"
                );
            }
            ensure!(
                hash(&fs::read(current)?) == *expected,
                "source hash mismatch: {name}"
            );
        }
        let root = path.parent().context("manifest parent missing")?;
        let actor: ActorInputs =
            serde_json::from_slice(&artifact(root, &m.actor_inputs, "actor-inputs.json")?)?;
        let evaluator: EvaluatorLabels = serde_json::from_slice(&artifact(
            root,
            &m.evaluator_labels,
            "evaluator-labels.json",
        )?)?;
        ensure!(
            actor.adapter_version == 1
                && evaluator.adapter_version == 1
                && evaluator.annotation_unit == "synthetic-unit",
            "unsupported adapter/scorer unit"
        );
        ensure!(actor.sources.len() == p.records, "source count mismatch");
        for (i, s) in actor.sources.iter().enumerate() {
            ensure!(
                s.ordinal == i
                    && s.source_id == format!("unit-{i:06}")
                    && s.turn_id == s.source_id
                    && s.group_id == "synthetic-regression"
                    && s.conversation_id == "synthetic-conversation"
                    && s.session_id == "synthetic-session"
                    && s.role == "user"
                    && s.source_timestamp == TIME
                    && s.summary == format!("fixture token{i}")
                    && s.content_sha256 == hash(s.summary.as_bytes()),
                "source provenance/ordinal/content mismatch"
            );
        }
        let queries: BTreeSet<_> = (0..p.records)
            .step_by(10)
            .map(|i| format!("query{i}"))
            .collect();
        ensure!(
            evaluator.relevant.keys().all(|q| queries.contains(q)),
            "unknown evaluator query"
        );
        let ids: BTreeSet<_> = actor.sources.iter().map(|s| s.source_id.as_str()).collect();
        for labels in evaluator.relevant.values() {
            ensure!(
                labels.iter().all(|s| ids.contains(s.as_str()))
                    && labels.iter().collect::<BTreeSet<_>>().len() == labels.len(),
                "unknown/duplicate evaluator source reference"
            );
        }
        ensure!(
            !evaluator.label_canary.is_empty(),
            "missing evaluator separation canary"
        );
        Ok(Self {
            manifest: m,
            environment_hash: hash(&bytes),
            sources: actor.sources,
            evaluator,
        })
    }

    pub fn relevant_ordinals(&self, query: &str) -> Option<Vec<usize>> {
        self.evaluator.relevant.get(query).map(|labels| {
            self.sources
                .iter()
                .filter(|s| labels.contains(&s.source_id))
                .map(|s| s.ordinal)
                .collect()
        })
    }

    pub fn mapping(&self, ordinal: usize, memory: recalld::model::MemoryId) -> Value {
        let source = &self.sources[ordinal];
        json!({"sourceMappingVersion":1,"memoryId":memory,"sourceIds":[source.source_id],
            "datasetRevision":self.manifest.actor_inputs.sha256,"source":source,
            "transformation":"raw_synthetic_unit","characterSpans":null,"extractor":null,
            "promptSha256":null,"outputSha256":source.content_sha256})
    }

    pub fn trace(&self, episode: &str, step: &str, action: Value, outcome: Value) -> Value {
        json!({"traceSchemaVersion":1,"runId":self.environment_hash,"episodeId":episode,
            "groupId":"synthetic-regression","stepId":step,"environmentHash":self.environment_hash,
            "track":TRACK,"simulatedTimeMs":TIME.to_string(),"action":action,"outcome":outcome,
            "readGeneration":null,"readiness":{"available":false,"reason":"coherent core/barrier not joined"},
            "snapshot":null,"model":null,"tokens":null,"reward":null})
    }

    pub fn replay_context(&self) -> ReplayContext {
        ReplayContext {
            profile: self.manifest.embedding.clone(),
            environment_hash: self.environment_hash.clone(),
        }
    }
}

/// Actor provider receives this projection, never the evaluator-bearing manifest object.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayContext {
    profile: Value,
    environment_hash: String,
}

impl ReplayContext {
    pub fn key(&self, namespace: &str, text: &str) -> String {
        hash(
            &serde_json::to_vec(
                &json!({"namespace":namespace,"text":text,"role":"actor-query",
            "profile":self.profile,"environmentHash":self.environment_hash}),
            )
            .unwrap(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_keys_bind_complete_actor_inputs_and_environment() {
        let base = ReplayContext {
            profile: embedding(16),
            environment_hash: hash(b"synthetic-environment-a"),
        };
        let key = base.key("fixture", "query0");
        assert_ne!(key, base.key("foreign", "query0"));
        assert_ne!(key, base.key("fixture", "query0 "));
        let mut changed = base.clone();
        changed.environment_hash = hash(b"synthetic-environment-b");
        assert_ne!(key, changed.key("fixture", "query0"));
        changed = base.clone();
        changed.profile = embedding(32);
        assert_ne!(key, changed.key("fixture", "query0"));
        let actor = serde_json::to_value(base).unwrap();
        assert_eq!(actor.as_object().unwrap().len(), 2);
        assert!(actor.get("evaluator").is_none());
    }
}
