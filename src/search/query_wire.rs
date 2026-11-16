//! Actual prepared query consumer; public version admission is separate.
use super::*;
use crate::serialization::contract::CommitState;
use crate::serialization::query::QueryRequest;
use crate::serialization::query_response::{QueryReply, project};

impl QueryEngine {
    pub(crate) async fn similar_contract(
        &self,
        request: crate::serialization::query::SimilarRequest,
    ) -> QueryReply {
        let accepted = match self.accept_work(None) {
            Ok(work) => work,
            Err(error) => return QueryReply::failure(error),
        };
        let mut captured = None;
        let result = async {
            let access = request.options.record_access;
            let all = request.options.scope == SimilarityScope::AllCompatible;
            let selected = self
                .prepare_similar_selection(
                    request.id,
                    request.namespace.as_deref(),
                    request.options,
                    accepted.as_ref(),
                )
                .await?;
            let mut reply = crate::serialization::query_response::project_similarity(
                &selected,
                all,
                request.explain,
            )?;
            captured = Some((reply.envelope.scope.clone(), selected.read_generation));
            if !reply.fits_before_observation(if access { selected.rows.len() } else { 0 }) {
                return Ok(reply.oversize());
            }
            let receipts = self
                .record_selected_similarity(&selected, accepted.as_ref())
                .await?;
            reply.envelope.receipt = (receipts.len() == 1).then(|| receipts[0].clone());
            reply.observation_receipts = receipts;
            Ok::<_, SearchError>(reply)
        }
        .await;
        let mut reply = match result {
            Ok(reply) => reply,
            Err(error) => QueryReply::failure(error),
        };
        if reply.envelope.scope.is_none()
            && let Some((scope, at)) = captured
        {
            reply.envelope.scope = scope;
            reply.envelope.read_generation = Some(at);
        }
        self.finish_contract_query(
            accepted,
            reply,
            #[cfg(test)]
            None,
        )
        .await
    }
    pub(crate) async fn query_contract(&self, request: QueryRequest) -> QueryReply {
        if self.rif_mode != RifMode::Off {
            return QueryReply::failure(SearchError::Unavailable(
                "operator RIF modes are not available through prepared queries".into(),
            ));
        }
        let accepted = match self.accept_work(None) {
            Ok(work) => work,
            Err(error) => return QueryReply::failure(error),
        };
        let mut captured = None;
        let result = async {
            // Validate supplied bytes against canonical identity, never width.
            let asserted = self
                .namespace_resolver
                .resolve(&request.query.namespace)
                .await?;
            if let Some(id) = &request.embedding_profile_id {
                let schema = asserted.schema.as_ref().ok_or_else(|| {
                    SearchError::InvalidEmbedding(
                        "explicit profile requires a canonical Bound namespace".into(),
                    )
                })?;
                schema.validate(&asserted).map_err(|_| {
                    SearchError::Unavailable("invalid canonical namespace schema".into())
                })?;
                match &schema.embedding_binding {
                    crate::model::schema::EmbeddingBinding::Bound { profile_id, .. }
                        if profile_id == id => {}
                    _ => return Err(SearchError::EmbeddingProfileMismatch),
                }
            }
            let prepared = self
                .prepare_coherent_payload_with_semantics(
                    request.query.clone(),
                    RetrievalSemantics::V2(request.version_mode),
                    request.include_embeddings,
                    request.include_neighbors,
                    accepted.as_ref(),
                )
                .await?
                .ok_or_else(|| {
                    SearchError::Unavailable("coherent query owner unavailable".into())
                })?;
            let payload = prepared
                .payload
                .as_ref()
                .ok_or_else(|| SearchError::Unavailable("coherent payload unavailable".into()))?;
            if payload.namespace.id != asserted.id || payload.namespace.schema != asserted.schema {
                return Err(SearchError::Unavailable(
                    "namespace changed during query admission".into(),
                ));
            }
            let mut reply = project(&prepared, request.explain)?;
            captured = Some((reply.envelope.scope.clone(), prepared.read_generation));
            let exposures = prepared.exposures(request.include_neighbors)?;
            if !reply.fits_before_observation(if request.query.record_access {
                exposures.len()
            } else {
                0
            }) {
                return Ok(reply.oversize());
            }
            if request.query.record_access {
                let receipts = self
                    .record_exposures_at_with_origin_work(
                        &exposures,
                        crate::storage::control::ExposureScope {
                            namespace: payload.namespace.name.clone(),
                            source: None,
                        },
                        prepared.evaluated_at,
                        accepted.as_ref(),
                        2,
                    )
                    .await?;
                reply.envelope.receipt = (receipts.len() == 1).then(|| receipts[0].clone());
                reply.observation_receipts = receipts;
            }
            Ok::<_, SearchError>(reply)
        }
        .await;
        let mut reply = match result {
            Ok(reply) => reply,
            Err(error) => QueryReply::failure(error),
        };
        if reply.envelope.scope.is_none()
            && let Some((scope, at)) = captured
        {
            reply.envelope.scope = scope;
            reply.envelope.read_generation = Some(at);
        }
        self.finish_contract_query(
            accepted,
            reply,
            #[cfg(test)]
            request.before_barrier,
        )
        .await
    }

    async fn finish_contract_query(
        &self,
        accepted: Option<crate::storage::work::WorkTicket>,
        mut reply: QueryReply,
        #[cfg(test)] before_barrier: Option<fn(&crate::derived_completion::DerivedCompletion)>,
    ) -> QueryReply {
        // Unknown workers stay tracked; no assumed root completion/self-drain.
        if reply
            .envelope
            .error
            .as_ref()
            .is_some_and(|e| e.commit_state == CommitState::Unknown)
        {
            drop(accepted);
            return reply;
        }
        self.finish_work(accepted);
        #[cfg(test)]
        if let Some(hook) = before_barrier
            && let Some((owner, _)) = &self.coherent_owner
        {
            hook(owner);
        }
        if reply.http_status == 200 && !reply.observation_receipts.is_empty() {
            let receipt = reply
                .observation_receipts
                .last()
                .expect("nonempty receipts");
            let target = crate::storage::control::GenerationRef {
                store_id: receipt.store_id,
                generation: receipt.committed_generation,
            };
            let result = if let Some((owner, _)) = &self.coherent_owner {
                owner
                    .readiness_barrier(
                        target,
                        std::time::Instant::now() + std::time::Duration::from_secs(30),
                    )
                    .await
            } else {
                Err(crate::storage::StorageError::RecoveryRequired(
                    "query publication owner unavailable".into(),
                ))
            };
            match result {
                Ok(ready) => reply.envelope.readiness = Some(ready.into()),
                Err(error) => {
                    let mut failed = QueryReply::failure(SearchError::Observation(
                        crate::decay::observation::ObservationError::not_ready(
                            reply.observation_receipts,
                            error,
                        ),
                    ));
                    failed.envelope.read_generation = reply.envelope.read_generation;
                    failed.envelope.scope = reply.envelope.scope;
                    return failed;
                }
            }
        }
        if !reply.fits_transport() {
            return reply.oversize();
        }
        reply
    }
}
