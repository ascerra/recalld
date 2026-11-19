//! Offline repair adapter; never activates a candidate or starts a daemon.
//! `--config CONFIG plan SOURCE --manifest PLAN.json` makes no provider calls.
//! `--config CONFIG apply SOURCE DEST --manifest PLAN.json --approved-drop-ids DROP.json --approved-repair-ids REPAIR.json`
//! uses only the configured local Ollama document provider. Inspect DEST/candidate
//! before running the published runtime's ordinary schema upgrade against it.
use clap::{Parser, Subcommand};
use recalld::{
    config::{RecalldConfig, loader},
    embedding::{self, EmbeddingProvider},
    model::{MemoryId, schema::EmbeddingProfile},
    storage::legacy_vector_repair::{self as repair, RepairDocument, RepairPlan},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Parser)]
struct Args {
    /// Explicit operator configuration; never discover a home config implicitly.
    #[arg(long)]
    config: PathBuf,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Plan {
        source: PathBuf,
        #[arg(long)]
        manifest: PathBuf,
    },
    Apply {
        source: PathBuf,
        destination: PathBuf,
        #[arg(long)]
        manifest: PathBuf,
        /// JSON array of the exact approved contentless memory IDs. No wildcard.
        #[arg(long)]
        approved_drop_ids: PathBuf,
        /// JSON array of the exact approved memories to regenerate.
        #[arg(long)]
        approved_repair_ids: PathBuf,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    plan: RepairPlan,
    // Describes this repair's generator, not the historical namespace identity.
    provider: EmbeddingProfile,
    endpoint: String,
}

fn local_endpoint(config: &RecalldConfig) -> Result<String> {
    if !matches!(
        config.embedding.provider,
        recalld::config::EmbeddingProvider::Ollama
    ) {
        return Err(
            "repair requires the configured local Ollama provider; no provider fallback".into(),
        );
    }
    let url = reqwest::Url::parse(&config.embedding.base_url)?;
    let host = url.host_str().unwrap_or("");
    let loopback = host
        .trim_matches(['[', ']'])
        .parse::<std::net::IpAddr>()
        .is_ok_and(|ip| ip.is_loopback());
    if !matches!(url.scheme(), "http" | "https")
        || !(loopback || matches!(host, "localhost" | "host.docker.internal"))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || config.embedding.model_name.ends_with(":cloud")
    {
        return Err("repair requires a local Ollama endpoint/model without URL credentials".into());
    }
    Ok(url.as_str().trim_end_matches('/').to_owned())
}

async fn provider(config: &RecalldConfig) -> Result<(String, Box<dyn EmbeddingProvider>)> {
    let endpoint = local_endpoint(config)?;
    let provider = embedding::build_provider(
        &embedding::EmbeddingConfig {
            provider: embedding::ProviderType::Ollama,
            dimensions: config.embedding.dimensions,
            model: config.embedding.model_name.clone(),
            api_key: None,
            base_url: Some(endpoint.clone()),
            batch_size: Some(config.embedding.batch_size),
            timeout_secs: None,
            region: None,
            cache_embeddings: false,
            cache_max_entries: 0,
        },
        &config.embedding.document_prefix,
        &config.embedding.query_prefix,
    )
    .await?;
    Ok((endpoint, provider))
}

fn ids(values: &[MemoryId]) -> Result<BTreeSet<MemoryId>> {
    let set: BTreeSet<_> = values.iter().copied().collect();
    if set.len() != values.len() {
        return Err("duplicate memory ID in manifest/approval".into());
    }
    Ok(set)
}
fn validate_approval(
    manifest: &Manifest,
    approved: &[MemoryId],
    approved_repairs: &[MemoryId],
) -> Result<()> {
    if manifest.version != 1
        || ids(approved)? != ids(&manifest.plan.drop_ids)?
        || ids(approved_repairs)? != ids(&manifest.plan.repair_ids)?
    {
        return Err("approved drop/repair IDs do not exactly match this repair manifest".into());
    }
    if !ids(&manifest.plan.drop_ids)?.is_disjoint(&ids(&manifest.plan.repair_ids)?) {
        return Err("drop and repair IDs overlap".into());
    }
    manifest.provider.validate()?;
    Ok(())
}

async fn regenerate(
    manifest: &Manifest,
    approved: &[MemoryId],
    approved_repairs: &[MemoryId],
    documents: Vec<RepairDocument>,
    provider: &dyn EmbeddingProvider,
) -> Result<BTreeMap<MemoryId, Vec<f32>>> {
    validate_approval(manifest, approved, approved_repairs)?;
    if provider.profile().as_ref() != Some(&manifest.provider) {
        return Err("configured provider profile differs from the approved manifest".into());
    }
    let document_ids: Vec<_> = documents.iter().map(|d| d.id).collect();
    if ids(&document_ids)? != ids(&manifest.plan.repair_ids)? {
        return Err("source documents do not exactly match the approved repair IDs".into());
    }
    // Validate the entire set before the first request, including namespaces
    // which cannot share this one explicitly configured generator.
    for document in &documents {
        if document.dimension as usize != provider.dimensions()
            || document.dimension != manifest.provider.dimension
            || document.document.trim().is_empty()
        {
            return Err(
                "repair document dimensions/content incompatible with configured provider".into(),
            );
        }
    }
    let mut vectors = BTreeMap::new();
    for document in documents {
        // Storage constructs the existing document-v1 recipe; embed (not
        // embed_query) applies the active configured contextual document prefix.
        let mut vector = provider.embed(&document.document).await?;
        embedding::validation::normalize_vector(&mut vector, document.dimension as usize)?;
        vectors.insert(document.id, vector);
    }
    Ok(vectors)
}

fn write_manifest(path: &Path, manifest: &Manifest) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(&serde_json::to_vec_pretty(manifest)?)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}

async fn run(args: Args) -> Result<()> {
    let loaded = loader::load_config(Some(&args.config), &loader::CliOverrides::default())
        .map_err(|errors| format!("configuration rejected: {errors:?}"))?;
    let (endpoint, provider) = provider(&loaded.config).await?;
    match args.command {
        Command::Plan { source, manifest } => {
            let plan = repair::plan(&source)?;
            let provider = provider.profile().ok_or("provider has no profile")?;
            let manifest_value = Manifest {
                version: 1,
                plan,
                provider,
                endpoint,
            };
            write_manifest(&manifest, &manifest_value)?;
            println!(
                "{}",
                serde_json::json!({"manifest":manifest,"repair_count":manifest_value.plan.repair_ids.len(),"drop_count":manifest_value.plan.drop_ids.len(),"provider_calls":0})
            );
        }
        Command::Apply {
            source,
            destination,
            manifest,
            approved_drop_ids,
            approved_repair_ids,
        } => {
            let manifest: Manifest = serde_json::from_slice(&fs::read(manifest)?)?;
            let approved: Vec<MemoryId> = serde_json::from_slice(&fs::read(approved_drop_ids)?)?;
            let approved_repairs: Vec<MemoryId> =
                serde_json::from_slice(&fs::read(approved_repair_ids)?)?;
            validate_approval(&manifest, &approved, &approved_repairs)?;
            if manifest.endpoint != endpoint
                || provider.profile().as_ref() != Some(&manifest.provider)
            {
                return Err("active embedding configuration changed since plan".into());
            }
            match fs::symlink_metadata(&destination) {
                Ok(_) => return Err("destination must be a fresh, nonexistent workspace".into()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
            // documents rechecks the full approved source fingerprint before
            // exposing provider inputs. apply rechecks again before any repair.
            let documents = repair::documents(&source, &manifest.plan)?;
            let vectors = regenerate(
                &manifest,
                &approved,
                &approved_repairs,
                documents,
                provider.as_ref(),
            )
            .await?;
            let result = repair::apply(&source, &destination, &manifest.plan, &vectors)?;
            println!(
                "{}",
                serde_json::json!({"workspace":destination,"candidate":destination.join("candidate"),"backup":destination.join("backup"),"records":result.records,"activated":false})
            );
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(error) = run(Args::parse()).await {
        eprintln!("Repair failed; source not activated or replaced: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn remote_and_paid_providers_are_rejected_before_construction() {
        let mut config = RecalldConfig::default();
        config.embedding.provider = recalld::config::EmbeddingProvider::OpenAI;
        assert!(local_endpoint(&config).is_err());
        config.embedding.provider = recalld::config::EmbeddingProvider::Ollama;
        for endpoint in [
            "https://example.com",
            "http://localhost@remote.invalid",
            "http://user:secret@localhost:11434",
        ] {
            config.embedding.base_url = endpoint.into();
            assert!(local_endpoint(&config).is_err());
        }
        config.embedding.base_url = "http://host.docker.internal:11434".into();
        assert!(local_endpoint(&config).is_ok());
    }

    #[tokio::test]
    async fn document_prefix_exact_ids_and_preflight_use_actual_local_provider() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        let app = axum::Router::new().route(
            "/api/embed",
            axum::routing::post(move |axum::Json(value): axum::Json<serde_json::Value>| {
                captured.lock().unwrap().push(value);
                async { axum::Json(serde_json::json!({"embeddings":[[1.,0.,0.,0.]]})) }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut config = RecalldConfig::default();
        config.embedding.provider = recalld::config::EmbeddingProvider::Ollama;
        config.embedding.base_url = format!("http://{}", listener.local_addr().unwrap());
        config.embedding.dimensions = 4;
        config.embedding.model_name = "synthetic-local-repair".into();
        config.embedding.document_prefix = "document: ".into();
        config.embedding.query_prefix = "query: ".into();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let (endpoint, provider) = provider(&config).await.unwrap();
        let id: MemoryId =
            serde_json::from_value(serde_json::json!("00000000-0000-4000-8000-000000000001"))
                .unwrap();
        let drop_id: MemoryId =
            serde_json::from_value(serde_json::json!("00000000-0000-4000-8000-000000000002"))
                .unwrap();
        let manifest = Manifest {
            version: 1,
            endpoint,
            provider: provider.profile().unwrap(),
            plan: RepairPlan {
                version: 1,
                inventory: BTreeMap::new(),
                drop_ids: vec![drop_id],
                repair_ids: vec![id],
                records: 2,
            },
        };
        let doc = |dimension| RepairDocument {
            id,
            namespace_id: 0,
            dimension,
            document: "summary\n\nfull text tag".into(),
        };
        assert!(
            regenerate(&manifest, &[], &[id], vec![doc(4)], provider.as_ref())
                .await
                .is_err()
        );
        assert!(
            regenerate(&manifest, &[drop_id], &[], vec![doc(4)], provider.as_ref())
                .await
                .is_err()
        );
        assert!(
            regenerate(
                &manifest,
                &[drop_id],
                &[id],
                vec![doc(3)],
                provider.as_ref()
            )
            .await
            .is_err()
        );
        assert!(
            regenerate(
                &manifest,
                &[drop_id],
                &[id],
                vec![doc(4), doc(4)],
                provider.as_ref()
            )
            .await
            .is_err()
        );
        assert!(requests.lock().unwrap().is_empty());
        let vectors = regenerate(
            &manifest,
            &[drop_id],
            &[id],
            vec![doc(4)],
            provider.as_ref(),
        )
        .await
        .unwrap();
        assert_eq!(vectors.keys().copied().collect::<Vec<_>>(), vec![id]);
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0]["input"],
            serde_json::json!(["document: summary\n\nfull text tag"])
        );
        assert_eq!(requests[0]["model"], "synthetic-local-repair");
        server.abort();
    }
}
