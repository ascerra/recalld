//! Legacy namespace creation participates in the existing owned publication path.
use super::*;
use crate::model::NamespaceConfig;
use crate::storage::control::{MutationContext, MutationOutcome, MutationScope, WriteReceipt};

#[derive(Debug, thiserror::Error)]
pub(crate) enum CatalogCreateError {
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error("namespace worker outcome unknown; reconcile before retry")]
    WorkerUnknown,
}

impl DerivedCompletion {
    pub(crate) async fn create_legacy_namespace(
        &self,
        config: NamespaceConfig,
        dimension: Option<u32>,
        clock: Arc<dyn crate::time::Clock>,
    ) -> Result<NamespaceConfig, CatalogCreateError> {
        let root = self.accept_work(None)?;
        let result = self
            .create_catalog_accepted(config, dimension, clock, &root)
            .await;
        if !matches!(result, Err(CatalogCreateError::WorkerUnknown)) {
            self.finish_known_work(root);
        }
        result
    }

    async fn create_catalog_accepted(
        &self,
        mut config: NamespaceConfig,
        dimension: Option<u32>,
        clock: Arc<dyn crate::time::Clock>,
        root: &WorkTicket,
    ) -> Result<NamespaceConfig, CatalogCreateError> {
        let lease = publication_gate(&self.storage).await?.read_owned().await;
        let child = root.child().map_err(StorageError::from)?;
        let storage = self.storage.clone();
        let (config, receipt): (_, WriteReceipt) = tokio::task::spawn_blocking(move || {
            // Cancellation cannot release a worker's lease or accepted child.
            let _lease = lease;
            let mut storage = storage
                .write()
                .map_err(|_| CatalogCreateError::WorkerUnknown)?;
            #[cfg(test)]
            let _fault = CatalogFault::enter(
                storage
                    .meta_store()
                    .catalog_test_fault
                    .load(std::sync::atomic::Ordering::SeqCst),
            );
            let result = (|| {
                config.embedding_dim = match dimension {
                    Some(dim) => dim,
                    None => storage
                        .get_namespace_by_name("default")?
                        .map(|ns| ns.embedding_dim)
                        .unwrap_or(1536),
                };
                config.created_at = clock.now_millis();
                let ctx = MutationContext {
                    scope: MutationScope::Catalog,
                    ..MutationContext::unkeyed(NamespaceId::new(1))
                };
                let receipt =
                    storage.create_namespace_with_receipt_and_work(&ctx, &config, Some(&child))?;
                let MutationOutcome::NamespaceCreated { namespace_id, .. } = receipt.outcome else {
                    return Err(StorageError::CommittedMutation {
                        receipt: Box::new(receipt),
                        source: Box::new(StorageError::InvalidControl(
                            "namespace create receipt outcome mismatch".into(),
                        )),
                    });
                };
                config.id = namespace_id;
                Ok((config, receipt))
            })();
            storage.meta_store().finish_work(child);
            result.map_err(CatalogCreateError::Storage)
        })
        .await
        .map_err(|_| CatalogCreateError::WorkerUnknown)??;
        // The worker has released its read lease. Completion owns the exclusive
        // lease and actual publishers; no generic generation acknowledgement.
        self.complete_with_work(Some(root))
            .await
            .map_err(|source| StorageError::CommittedMutation {
                receipt: Box::new(receipt),
                source: Box::new(source),
            })?;
        Ok(config)
    }
}

#[cfg(test)]
struct CatalogFault;
#[cfg(test)]
impl CatalogFault {
    fn enter(mode: u8) -> Self {
        if mode == 3 {
            panic!("injected lost catalog worker");
        }
        if mode == 2 {
            crate::storage::catalog::FAILURE.with(|v| v.set(Some("commit_unknown")));
        }
        Self
    }
}
#[cfg(test)]
impl Drop for CatalogFault {
    fn drop(&mut self) {
        crate::storage::catalog::FAILURE.with(|v| v.set(None));
    }
}
