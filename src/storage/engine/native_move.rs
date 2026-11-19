use super::*;
use crate::storage::native_move::{CapturedNativeMove, NativeMoveInput, SharedMoveProgress};
use crate::storage::work::WorkTicket;

impl RedbStorageEngine {
    pub(crate) fn capture_native_move(
        &self,
        input: NativeMoveInput,
        parent: Option<&WorkTicket>,
    ) -> Result<CapturedNativeMove, StorageError> {
        let mut capture = self.meta_store.capture_native_move(input, parent)?;
        for record in &capture.records {
            let full_text = if record.text_length == 0 {
                None
            } else {
                Some(self.text_store.read(TextRef {
                    file_offset: record.text_offset,
                    length: record.text_length,
                })?)
            };
            capture
                .documents
                .push(crate::storage::vector_certification::retained_document(
                    record,
                    full_text.as_deref(),
                )?);
        }
        Ok(capture)
    }

    pub(crate) fn move_native_prepared(
        &mut self,
        capture: &CapturedNativeMove,
        generated: &[crate::embedding::runtime::GeneratedEmbedding],
        progress: &SharedMoveProgress,
        parent: Option<&WorkTicket>,
    ) -> Result<WriteReceipt, StorageError> {
        if let Some(receipt) = self.meta_store.lookup_native_move(&capture.input, parent)? {
            return Ok(receipt);
        }
        let guard = self.meta_store.native_guard_with_work(parent)?;
        let vectors = &mut self.vector_manager;
        let mut staged = false;
        let result = self.meta_store.commit_native_move(
            &guard,
            capture,
            generated,
            progress,
            |records, normalized| {
                let store = vectors.get_mut(capture.destination.id).ok_or(
                    StorageError::NamespaceNotFound(capture.destination.id.get()),
                )?;
                staged = true;
                for (record, values) in records.iter_mut().zip(normalized) {
                    let slot = store.insert_vector(values)?;
                    record.vector_slot = slot;
                    record
                        .schema
                        .as_mut()
                        .unwrap()
                        .vector
                        .as_mut()
                        .unwrap()
                        .slot = slot;
                }
                store.sync()?;
                crate::storage::metadata::effects::crash_point("after_move_payload_sync");
                Ok(())
            },
        );
        if result.is_err() && staged {
            // Even an I/O failure may follow commit. Never free destination
            // slots here; exclusive recovery validates owners before cleanup.
            self.meta_store.mark_unavailable();
        }
        let receipt = result?;
        if !receipt.replayed {
            effects::apply_pending(
                &self.meta_store,
                &guard,
                &self.edge_store,
                Some(&mut self.vector_manager),
            )
            .map_err(|source| StorageError::CommittedMutation {
                receipt: Box::new(receipt.clone()),
                source: Box::new(source),
            })?;
        }
        Ok(receipt)
    }
}
