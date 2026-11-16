use super::{
    CliError, Result,
    commands::{PolicyAction, PolicyArgs},
    print_out,
};
use crate::storage::inherited_policy::{
    AdoptInheritedDecay, adopt_inherited_decay, preview_inherited_decay,
};
use std::io::Read;

pub(super) fn run(args: PolicyArgs) -> Result<()> {
    let result = match args.action {
        PolicyAction::PreviewInherited {
            store,
            namespace_id,
            multiplier,
            expected_store_id,
            expected_generation,
        } => {
            let preview = preview_inherited_decay(
                &store,
                crate::model::NamespaceId::new(namespace_id),
                multiplier,
                crate::storage::control::GenerationRef {
                    store_id: expected_store_id,
                    generation: expected_generation,
                },
                &crate::time::SystemClock,
            )
            .map_err(|e| CliError::Other(e.to_string()))?;
            serde_json::to_value(preview)?
        }
        PolicyAction::AdoptInherited { store, request } => {
            // Bound before allocation/parsing. The request is fixed-size evidence,
            // not a manifest of memory payload or an unbounded list of IDs.
            const LIMIT: u64 = 16 * 1024;
            let file = std::fs::File::open(request).map_err(|e| CliError::Other(e.to_string()))?;
            let mut bytes = Vec::new();
            file.take(LIMIT + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| CliError::Other(e.to_string()))?;
            if bytes.len() as u64 > LIMIT {
                return Err(CliError::Other("adoption request exceeds 16 KiB".into()));
            }
            let request: AdoptInheritedDecay = serde_json::from_slice(&bytes)?;
            let receipt = adopt_inherited_decay(&store, &request)
                .map_err(|e| CliError::Other(e.to_string()))?;
            serde_json::json!({"receipt":receipt,"readiness":"reopen_required"})
        }
    };
    print_out(&format!("{}\n", serde_json::to_string_pretty(&result)?));
    Ok(())
}
