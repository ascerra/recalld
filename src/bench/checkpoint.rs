//! Isolated legacy fixture checkpoints, never production retained-state restore.
//! The supervisor must terminate/join the old child before constructing a clone.
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::frozen::{FrozenControls, OwnedEvaluator};
use crate::config::RecalldConfig;
use crate::storage::control::GenerationRef;
use crate::storage::ids::{IdState, SeededIds};
use crate::storage::offline_snapshot::OfflineSnapshot;
use crate::storage::ownership::OwnedFileLock;
use crate::time::{Clock, ManualClock};

const TRACK: &str = "isolated-legacy-checkpoint-v1";
const CACHE_POLICY: &str = "empty-record-cache-rif-off-autolink-off";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Manifest {
    checkpoint_version: u32,
    track: String,
    generation: GenerationRef,
    logical_sha256: String,
    files_sha256: BTreeMap<String, String>,
    now_millis: i64,
    allocation: IdState,
    config: serde_json::Value,
    source_root: PathBuf,
    ancestor_roots: Vec<PathBuf>,
    executable_sha256: String,
    cargo_lock_sha256: String,
    cache_policy: String,
    index_policy: String,
}

fn check(deadline: Instant) -> anyhow::Result<()> {
    anyhow::ensure!(Instant::now() < deadline, "checkpoint deadline expired");
    Ok(())
}
fn digest(bytes: &[u8]) -> String {
    digest_array(Sha256::digest(bytes).into())
}
fn file_hash(path: &Path, deadline: Instant) -> anyhow::Result<String> {
    crate::storage::validation::require_file(path)?;
    let mut file = std::fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        check(deadline)?;
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(digest_array(hash.finalize().into()))
}
fn config_value(config: &RecalldConfig) -> anyhow::Result<serde_json::Value> {
    let mut config = config.clone();
    config.storage.data_dir = "<fresh-isolated-root>".into();
    Ok(serde_json::to_value(config)?)
}
fn supported(config: &RecalldConfig) -> bool {
    config.decay.disable_sweep && !config.rif.enabled && !config.graph.autolink_enabled
}
// Enumerate only this freshly created canonical stage; reject unknown files,
// symlinks and deeper paths. No source checkout or arbitrary live-root walk.
fn inventory(root: &Path, deadline: Instant) -> anyhow::Result<BTreeMap<String, String>> {
    let mut files = BTreeMap::new();
    for entry in std::fs::read_dir(root)? {
        check(deadline)?;
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("non UTF-8 path"))?;
        if name == "recalld.lock" {
            continue;
        }
        if entry.file_type()?.is_dir() {
            crate::model::validation::validate_namespace_name(&name)
                .map_err(|_| anyhow::anyhow!("invalid namespace path"))?;
            for child in std::fs::read_dir(entry.path())? {
                let child = child?;
                anyhow::ensure!(
                    child.file_name() == "vectors.dat",
                    "unknown canonical directory input"
                );
                files.insert(
                    format!("{name}/vectors.dat"),
                    file_hash(&child.path(), deadline)?,
                );
            }
        } else {
            anyhow::ensure!(entry.file_type()?.is_file(), "nonregular canonical input");
            // Text generations are selected by the validated metadata below.
            anyhow::ensure!(
                name == "meta.db"
                    || name == "edges.db"
                    || name == "fulltext.dat"
                    || crate::storage::metadata::text_generation::is_generation_filename(&name),
                "unknown canonical input: {name}"
            );
            files.insert(name, file_hash(&entry.path(), deadline)?);
        }
    }
    Ok(files)
}

impl OwnedEvaluator {
    pub(crate) fn checkpoint_supported(&self) -> bool {
        supported(self.core.config())
    }

    /// Capture under the one owned freeze. Publish a manifest only after the
    /// blocking worker joins successfully; a timed-out worker only owns TempDir.
    pub(crate) async fn checkpoint(&self, deadline: Instant) -> anyhow::Result<serde_json::Value> {
        anyhow::ensure!(
            self.checkpoint_supported(),
            "checkpoint requires explicit sweeps, RIF off and autolink off"
        );
        let config = config_value(self.core.config())?;
        let source_root = PathBuf::from(&self.core.config().storage.data_dir).canonicalize()?;
        let clock = self.controls.clock.clone();
        let cache = self.core.cache().clone();
        let runtime = tokio::runtime::Handle::current();
        let (stage, allocation, now_millis) = self
            .controls
            .capture(deadline, move |engine, frozen, _| {
                runtime.block_on(cache.clear_for_frozen_fixture());
                if cache.entry_count() != 0 {
                    return Err(crate::storage::StorageError::RecoveryRequired(
                        "cache did not drain".into(),
                    ));
                }
                let allocation = engine
                    .meta_store()
                    .frozen_id_state(frozen)?
                    .ok_or_else(|| {
                        crate::storage::StorageError::RecoveryRequired(
                            "owned allocation source required".into(),
                        )
                    })?;
                SeededIds::from_state(allocation.clone())?;
                let now = clock.now_millis();
                allocation.validate_clock(now)?;
                let stage = engine.stage_frozen_canonical(frozen, deadline)?;
                Ok((stage, allocation, now))
            })
            .await?;
        let manifest = Manifest {
            checkpoint_version: 1,
            track: TRACK.into(),
            generation: stage.generation,
            logical_sha256: digest_array(stage.logical_sha256),
            files_sha256: inventory(&stage.path(), deadline)?,
            now_millis,
            allocation,
            config,
            source_root,
            ancestor_roots: self.fixture_ancestors.clone(),
            executable_sha256: file_hash(&std::env::current_exe()?, deadline)?,
            cargo_lock_sha256: digest(include_bytes!("../../Cargo.lock")),
            cache_policy: CACHE_POLICY.into(),
            index_policy: "rebuild-all-four-from-validated-canonical-v1".into(),
        };
        let bytes = serde_json::to_vec_pretty(&manifest)?;
        check(deadline)?;
        let path = stage.directory.path().join("manifest.json");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        check(deadline)?;
        let _retained = stage.directory.keep();
        Ok(
            serde_json::json!({"manifest":path,"sha256":digest(&bytes),"committed":manifest.generation,"logicalHash":manifest.logical_sha256,"track":TRACK}),
        )
    }

    /// Fresh process only. Existing A1 owner locks prove the old engine and its
    /// retained workers released ownership. This never opens a second old core.
    pub async fn from_checkpoint(
        config: RecalldConfig,
        manifest_path: &Path,
        expected_sha256: &str,
    ) -> anyhow::Result<Self> {
        let deadline = Instant::now() + Duration::from_secs(30);
        anyhow::ensure!(supported(&config), "unsupported checkpoint configuration");
        crate::storage::validation::require_file(manifest_path)?;
        let mut bytes = Vec::new();
        std::fs::File::open(manifest_path)?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        anyhow::ensure!(
            bytes.len() <= 1024 * 1024 && digest(&bytes) == expected_sha256,
            "checkpoint manifest digest/size mismatch"
        );
        let manifest: Manifest = serde_json::from_slice(&bytes)?;
        anyhow::ensure!(
            manifest.checkpoint_version == 1
                && manifest.track == TRACK
                && manifest.cache_policy == CACHE_POLICY
                && manifest.index_policy == "rebuild-all-four-from-validated-canonical-v1",
            "unsupported checkpoint schema/policies"
        );
        anyhow::ensure!(
            manifest.config == config_value(&config)?,
            "checkpoint configuration mismatch"
        );
        anyhow::ensure!(
            manifest.executable_sha256 == file_hash(&std::env::current_exe()?, deadline)?
                && manifest.cargo_lock_sha256 == digest(include_bytes!("../../Cargo.lock")),
            "checkpoint build mismatch"
        );
        anyhow::ensure!(
            (0..(1_i64 << 48)).contains(&manifest.now_millis),
            "invalid checkpoint clock"
        );
        manifest.allocation.validate_clock(manifest.now_millis)?;
        let ids = SeededIds::from_state(manifest.allocation.clone())?;
        let checkpoint_dir = manifest_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("missing checkpoint directory"))?
            .canonicalize()?;
        let source = manifest.source_root.canonicalize()?;
        let parent = source
            .parent()
            .ok_or_else(|| anyhow::anyhow!("missing fixture parent"))?;
        anyhow::ensure!(
            checkpoint_dir.parent() == Some(parent) && source == manifest.source_root,
            "checkpoint/source outside fixture parent"
        );
        let destination = PathBuf::from(&config.storage.data_dir);
        anyhow::ensure!(
            !destination.try_exists()? && !destination.is_symlink(),
            "restore destination must be fresh"
        );
        anyhow::ensure!(
            destination
                .parent()
                .ok_or_else(|| anyhow::anyhow!("missing destination parent"))?
                .canonicalize()?
                == parent,
            "restore outside fixture parent"
        );
        let mut ancestor_roots = manifest.ancestor_roots.clone();
        anyhow::ensure!(ancestor_roots.len() < 64, "fixture ancestor limit exceeded");
        ancestor_roots.push(source.clone());
        ancestor_roots.sort();
        anyhow::ensure!(
            ancestor_roots.windows(2).all(|pair| pair[0] != pair[1]),
            "duplicate fixture ancestor"
        );
        let mut old_owner = Vec::new();
        for ancestor in &ancestor_roots {
            anyhow::ensure!(
                ancestor.parent() == Some(parent) && ancestor.canonicalize()? == *ancestor,
                "ancestor outside fixture parent"
            );
            let name = ancestor.file_name().unwrap().to_string_lossy();
            // Existing A1 locks only; no new registry or source initialization.
            for path in [
                parent.join(format!("{name}.recalld-owner.lock")),
                ancestor.join("recalld.lock"),
            ] {
                anyhow::ensure!(
                    std::fs::symlink_metadata(&path)?.file_type().is_file(),
                    "invalid owner lock"
                );
                old_owner.push(OwnedFileLock::acquire(
                    std::fs::OpenOptions::new().write(true).open(path)?,
                )?);
            }
        }
        let canonical = checkpoint_dir.join("canonical");
        anyhow::ensure!(
            std::fs::symlink_metadata(&canonical)?.file_type().is_dir(),
            "nonregular canonical root"
        );
        anyhow::ensure!(
            inventory(&canonical, deadline)? == manifest.files_sha256,
            "canonical file digest mismatch"
        );
        let snapshot = OfflineSnapshot::open(&canonical)?;
        anyhow::ensure!(
            snapshot.control().generation_ref() == manifest.generation
                && digest_array(crate::storage::logical_export::isolated_legacy_hash(
                    &snapshot, deadline
                )?) == manifest.logical_sha256,
            "canonical logical generation/hash mismatch"
        );
        drop(snapshot);
        let stage = tempfile::Builder::new()
            .prefix("recalld-reset-stage-")
            .tempdir_in(parent)?;
        let target = stage.path().join("canonical");
        std::fs::create_dir(&target)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o700))?;
        }
        for (name, expected) in &manifest.files_sha256 {
            let path = Path::new(name);
            anyhow::ensure!(
                path.components().all(|c| matches!(c, Component::Normal(_))),
                "invalid canonical path"
            );
            let output = target.join(path);
            std::fs::create_dir_all(output.parent().unwrap())?;
            check(deadline)?;
            std::fs::copy(canonical.join(path), &output)?;
            anyhow::ensure!(
                file_hash(&output, deadline)? == *expected,
                "clone changed during copy"
            );
        }
        let snapshot = OfflineSnapshot::open(&target)?;
        anyhow::ensure!(
            snapshot.control().generation_ref() == manifest.generation
                && digest_array(crate::storage::logical_export::isolated_legacy_hash(
                    &snapshot, deadline
                )?) == manifest.logical_sha256,
            "clone logical hash mismatch"
        );
        drop(snapshot);
        check(deadline)?;
        crate::storage::engine::rename_new_directory(&target, &destination)?;
        let clock = Arc::new(ManualClock::new(manifest.now_millis));
        let core = Arc::new(
            crate::Recalld::with_runtime_sources(config, clock.clone(), Box::new(ids)).await?,
        );
        let controls = FrozenControls::new(core.derived_owner(), clock);
        core.storage()
            .write()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?
            .fixture_owners = old_owner;
        let evaluator = Self {
            core,
            controls,
            fixture_ancestors: ancestor_roots,
        };
        let cache = evaluator.core.cache().clone();
        let runtime = tokio::runtime::Handle::current();
        evaluator
            .controls
            .capture(deadline, move |engine, frozen, _| {
                runtime.block_on(cache.clear_for_frozen_fixture());
                let stage = engine.stage_frozen_canonical(frozen, deadline)?;
                if stage.generation != manifest.generation
                    || digest_array(stage.logical_sha256) != manifest.logical_sha256
                    || engine.meta_store().frozen_id_state(frozen)? != Some(manifest.allocation)
                {
                    return Err(crate::storage::StorageError::RecoveryRequired(
                        "restored runtime differs from checkpoint".into(),
                    ));
                }
                Ok(())
            })
            .await?;
        Ok(evaluator)
    }
}
fn digest_array(bytes: [u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
