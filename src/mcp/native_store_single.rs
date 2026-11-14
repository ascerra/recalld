//! Typed single producer for the native wire consumer. Public legacy dispatch
//! is unchanged; a single operation never acquires catalog batch semantics.
use super::McpStorageAdapter;
use crate::storage::StorageError;
use crate::storage::control::{MutationContext, MutationOutcome, WriteReceipt};
use crate::storage::native_lifecycle::NativePolicyInputs;
use crate::storage::native_store::NativeStoreInput;
use crate::storage::work::WorkTicket;

pub(crate) struct NativeSingleResult {
    pub receipt: WriteReceipt,
    pub advisory_requested: bool,
    pub advisory: crate::mcp::native_advisory::NativeAdvisoryRead,
}

impl McpStorageAdapter {
    pub(crate) async fn store_native_single(
        &self,
        ctx: MutationContext,
        input: NativeStoreInput,
        policy: NativePolicyInputs,
        parent: Option<&WorkTicket>,
    ) -> Result<NativeSingleResult, StorageError> {
        let completion = self.batch_completion();
        let work = completion.accept_work(parent)?;
        let result = async {
            let receipt = completion
                .store_native_with_work(
                    ctx,
                    input,
                    self.embedding.clone(),
                    self.config.graph.clone(),
                    policy,
                    self.clock.clone(),
                    Some(&work),
                )
                .await?;
            let MutationOutcome::NativeMemoryInsertedWithLinks { transition } = &receipt.outcome
            else {
                return Err(StorageError::CommittedMutation {
                    receipt: Box::new(receipt),
                    source: Box::new(StorageError::InvalidControl(
                        "wrong native single outcome".into(),
                    )),
                });
            };
            let advisory_requested = transition.check_duplicates;
            let advisory = match self.storage.read() {
                Ok(engine) => crate::mcp::native_advisory::hydrate(
                    &engine,
                    transition.namespace_id,
                    transition.advisory_matches().as_deref(),
                ),
                Err(_) => crate::mcp::native_advisory::NativeAdvisoryRead {
                    report: None,
                    read_generation: None,
                    warnings: vec!["ADVISORY_UNAVAILABLE"],
                },
            };
            Ok(NativeSingleResult {
                receipt,
                advisory_requested,
                advisory,
            })
        }
        .await;
        completion.finish_known_work(work);
        result
    }
}
