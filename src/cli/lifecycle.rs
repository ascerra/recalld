//! `recalld lifecycle preview|migrate`: move namespaces to the explicit-feedback
//! lifecycle offline. The source store is never changed; `migrate` writes a
//! validated copy to a fresh directory, which the operator puts in place.
use crate::config::RecalldConfig;
use crate::decay::DecayConfig;
use crate::model::{NamespaceConfig, schema::LifecycleVersion};
use crate::storage::control::{GenerationRef, MutationContext, MutationScope};
use crate::storage::inherited_policy::{
    AdoptInheritedDecay, adopt_inherited_decay, preview_inherited_decay,
};
use crate::storage::lifecycle_migration::{
    LifecycleMigrationOptions, LifecyclePreview, migrate_lifecycle, preview_lifecycle,
};
use crate::storage::schema_migration::inspect_schema;
use crate::time::{Clock, SystemClock};
use clap::{Args, Subcommand};
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const TARGET: LifecycleVersion = LifecycleVersion::ExplicitFeedbackV2;

#[derive(Args, Debug)]
#[group(required = true, multiple = false)]
pub struct Selection {
    /// The namespace to move to explicit feedback
    #[arg(long)]
    pub namespace: Option<String>,
    /// Every namespace still on the legacy lifecycle
    #[arg(long)]
    pub all: bool,
}

#[derive(Subcommand, Debug)]
pub enum LifecycleCommand {
    /// Read-only: what the next decay sweep does to each namespace to be moved
    Preview {
        #[command(flatten)]
        select: Selection,
    },
    /// Write a migrated, validated copy of the stopped store to a fresh directory
    Migrate {
        #[command(flatten)]
        select: Selection,
        /// New directory for the migrated store; the current store is not changed
        #[arg(long)]
        destination: PathBuf,
    },
}

/// Namespaces the selection moves, by name. Fails for an unknown name.
fn selected(namespaces: &[NamespaceConfig], select: &Selection) -> Result<Vec<String>> {
    let candidates = match &select.namespace {
        Some(name) => vec![find(namespaces, name)?],
        None => namespaces.iter().collect(),
    };
    Ok(candidates
        .into_iter()
        .filter(|ns| {
            ns.schema
                .as_ref()
                .is_some_and(|s| s.lifecycle_version != TARGET)
        })
        .map(|ns| ns.name.clone())
        .collect())
}

fn find<'a>(namespaces: &'a [NamespaceConfig], name: &str) -> Result<&'a NamespaceConfig> {
    namespaces
        .iter()
        .find(|ns| ns.name == name)
        .ok_or_else(|| format!("namespace {name:?} not found").into())
}

fn options(source: GenerationRef, ns: &NamespaceConfig) -> Result<LifecycleMigrationOptions> {
    let revision = ns
        .schema
        .as_ref()
        .ok_or("namespace has no lifecycle schema")?
        .policy_revision;
    Ok(LifecycleMigrationOptions {
        expected_source: source,
        namespace_id: ns.id,
        expected_policy_revision: revision,
        target: TARGET,
        idempotency_key: format!("recalld-lifecycle-{}-{}", ns.id.get(), revision.get()),
    })
}

/// Read-only counts for each selected namespace, evaluated now.
pub fn preview(
    config: &RecalldConfig,
    source: &Path,
    names: &[String],
) -> Result<Vec<LifecyclePreview>> {
    let inspection = inspect_schema(source)?;
    names
        .iter()
        .map(|name| {
            let ns = find(&inspection.namespaces, name)?;
            Ok(preview_lifecycle(
                source,
                &options(inspection.control.generation_ref(), ns)?,
                &SystemClock,
                &DecayConfig::default(),
                config.decay.decay_rate_multiplier,
                !config.decay.disable_sweep,
            )?)
        })
        .collect()
}

/// Copy `source` to `destination` with every named namespace on explicit
/// feedback. Each step is the receipted `migrate_lifecycle` into a fresh
/// directory under a private work directory next to `destination`; nothing
/// appears at `destination` unless every step and the final validation pass.
///
/// A namespace whose inherited decay multiplier was never recorded (one
/// migrated from 1.x) gets `multiplier` adopted with a receipt, because the
/// explicit lifecycle refuses to guess it.
// ponytail: one full store copy per namespace; fold the steps into one copy if stores get large.
pub fn migrate(
    source: &Path,
    destination: &Path,
    names: &[String],
    multiplier: f64,
    clock: &dyn Clock,
) -> Result<()> {
    if names.is_empty() {
        return Err("no namespace to migrate".into());
    }
    if destination.try_exists()? {
        return Err(format!(
            "{} already exists; choose a new directory",
            destination.display()
        )
        .into());
    }
    let parent = std::path::absolute(destination)?
        .parent()
        .ok_or("destination has no parent directory")?
        .canonicalize()?;
    let start = inspect_schema(source)?.control.generation_ref();
    let work = tempfile::Builder::new()
        .prefix(".recalld-lifecycle-")
        .tempdir_in(&parent)?;
    let mut current = source.to_path_buf();
    for (step, name) in names.iter().enumerate() {
        let inspection = inspect_schema(&current)?;
        let ns = find(&inspection.namespaces, name)?;
        let next = work.path().join(step.to_string());
        migrate_lifecycle(
            &current,
            &next,
            &options(inspection.control.generation_ref(), ns)?,
        )?;
        if ns.decay_rate_multiplier.is_none()
            && ns
                .schema
                .as_ref()
                .is_some_and(|s| s.inherited_decay.is_none())
        {
            let at = inspect_schema(&next)?.control.generation_ref();
            let preview = preview_inherited_decay(&next, ns.id, multiplier, at, clock)?;
            adopt_inherited_decay(
                &next,
                &AdoptInheritedDecay {
                    context: MutationContext {
                        store_id: Some(at.store_id),
                        scope: MutationScope::Namespace(ns.id),
                        idempotency_key: None,
                        expected_generation: Some(at.generation),
                        contract_version: 2,
                    },
                    preview,
                },
            )?;
        }
        if current != source {
            std::fs::remove_dir_all(&current)?;
        }
        current = next;
    }
    // Full canonical validation, including every policy receipt, as startup reads it.
    let result = inspect_schema(&current)?;
    for name in names {
        let ns = find(&result.namespaces, name)?;
        let schema = ns
            .schema
            .as_ref()
            .ok_or("namespace has no lifecycle schema")?;
        if schema.lifecycle_version != TARGET
            || (ns.decay_rate_multiplier.is_none() && schema.inherited_decay.is_none())
        {
            return Err(format!("namespace {name:?} did not migrate").into());
        }
    }
    if inspect_schema(source)?.control.generation_ref() != start {
        return Err(
            "the store changed during migration (is the daemon running?); \
                    nothing was written to the destination"
                .into(),
        );
    }
    if destination.try_exists()? {
        return Err(format!("{} appeared during migration", destination.display()).into());
    }
    std::fs::rename(&current, destination)?;
    crate::storage::fsync::fsync_dir(&parent)?;
    Ok(())
}

fn describe(p: &LifecyclePreview, name: &str) -> String {
    let mut line = format!(
        "{name}: {} memories; at the next decay sweep {} move to Summary, {} to Ghost, {} are deleted",
        p.record_count, p.full_to_summary, p.summary_to_ghost, p.ghost_to_delete
    );
    if p.missing_summary > 0 {
        line += &format!(
            " ({} more are due for Summary but have no summary)",
            p.missing_summary
        );
    }
    line
}

pub fn run(config: &RecalldConfig, command: LifecycleCommand) -> Result<()> {
    let source = PathBuf::from(&config.storage.data_dir);
    let (select, destination) = match command {
        LifecycleCommand::Preview { select } => (select, None),
        LifecycleCommand::Migrate {
            select,
            destination,
        } => (select, Some(destination)),
    };
    let names = selected(&inspect_schema(&source)?.namespaces, &select)?;
    if names.is_empty() {
        println!("Nothing to migrate: the selected namespaces already use explicit feedback.");
        return Ok(());
    }
    println!(
        "Moving to explicit feedback leaves every decay clock and stability as it is: \
         this is what the next sweep does either way. Afterwards, recalls no longer \
         restart a memory's clock; only reinforce_memory strengthens it."
    );
    for (name, p) in names.iter().zip(preview(config, &source, &names)?) {
        println!("  {}", describe(&p, name));
    }
    let Some(destination) = destination else {
        return Ok(());
    };
    migrate(
        &source,
        &destination,
        &names,
        config.decay.decay_rate_multiplier,
        &SystemClock,
    )?;
    println!(
        "\nMigrated {} namespace(s) into {}. {} is unchanged. To use the migrated store:\n\
         \n  recalld daemon stop\
         \n  mv {src} {src}.pre-lifecycle\
         \n  mv {dst} {src}\
         \n  recalld daemon start\
         \n\nKeep {src}.pre-lifecycle until you are happy; moving it back undoes the change.",
        names.len(),
        destination.display(),
        source.display(),
        src = source.display(),
        dst = destination.display(),
    );
    Ok(())
}
