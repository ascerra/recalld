use super::{CliError, Result, commands::ReembedVectorsArgs, print_out};
use crate::storage::vector_certification::{ReembedVectors, reembed_fresh_with_provider};
use serde::Deserialize;
use std::io::Read;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProviderInput {
    configuration: crate::embedding::EmbeddingConfig,
    document_prefix: String,
    query_prefix: String,
}
fn read<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Result<T> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| CliError::Other(e.to_string()))?
        .take(16 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| CliError::Other(e.to_string()))?;
    if bytes.len() > 16 * 1024 {
        return Err(CliError::Other(
            "offline request/configuration exceeds 16 KiB".into(),
        ));
    }
    Ok(serde_json::from_slice(&bytes)?)
}
pub(super) async fn run(args: ReembedVectorsArgs) -> Result<()> {
    let request: ReembedVectors = read(&args.request)?;
    let result = reembed_fresh_with_provider(&args.source, &args.destination, &request, async {
        let config: ProviderInput = read(&args.provider)
            .map_err(|e| crate::storage::StorageError::InvalidMutation(e.to_string()))?;
        // Execute the actual provider recipe; never reuse cached old output.
        if config.configuration.cache_embeddings {
            return Err(crate::storage::StorageError::InvalidMutation(
                "offline re-embedding requires cache_embeddings=false".into(),
            ));
        }
        crate::embedding::build_provider(
            &config.configuration,
            &config.document_prefix,
            &config.query_prefix,
        )
        .await
        .map_err(|e| crate::storage::StorageError::InvalidMutation(e.to_string()))
    })
    .await;
    let receipt = match result {
        Ok(receipt) => receipt,
        Err(crate::storage::StorageError::CommittedMutation { receipt, source }) => {
            print_out(&format!(
                "{}\n",
                serde_json::json!({"receipt": receipt, "readiness": "recovery_required", "error": source.to_string()})
            ));
            return Err(CliError::Other(
                "canonical cutover committed; inspect or retry the original request".into(),
            ));
        }
        Err(crate::storage::StorageError::CommitOutcomeUnknown {
            operation_id,
            source,
        }) => {
            print_out(&format!(
                "{}\n",
                serde_json::json!({"outcome": "unknown", "operationId": operation_id, "error": source.to_string()})
            ));
            return Err(CliError::Other(
                "cutover outcome unknown; retain the private stage and retry the original request"
                    .into(),
            ));
        }
        Err(error) => return Err(CliError::Other(error.to_string())),
    };
    print_out(&format!(
        "{}\n",
        serde_json::json!({"receipt": receipt, "readiness": "reopen_required"})
    ));
    Ok(())
}
