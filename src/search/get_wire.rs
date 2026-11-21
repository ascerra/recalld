//! Canonical point capture for the prepared get consumer. No ranked-read gate.
use super::*;
use crate::search::adapters::SharedGraphReader;
use crate::storage::StorageError;
use crate::storage::control::GenerationRef;
use crate::storage::engine::HydratedRecord;
use crate::storage::native_lifecycle::{ExposureIntent, LifecycleScope, NativePolicyInputs};
use crate::storage::work::WorkTicket;

type GraphLease = Arc<tokio::sync::OwnedRwLockReadGuard<crate::graph::RelationshipGraph>>;

struct PointSelection {
    at: GenerationRef,
    evaluated_at: i64,
    payload: Option<(NamespaceConfig, HydratedRecord)>,
    donors: Vec<CachedRecord>,
    intent: Option<ExposureIntent>,
}

enum PointAttempt {
    NeedsGraph(GenerationRef),
    Captured(PointSelection),
}

fn read_error(error: StorageError) -> SearchError {
    match error {
        StorageError::NamespaceMismatch { .. }
        | StorageError::InvalidNamespaceName { .. }
        | StorageError::NotFound(_) => SearchError::CanonicalRead(Box::new(error)),
        _ => SearchError::Unavailable(error.to_string()),
    }
}

fn exposed_phase(phase: DecayPhase) -> bool {
    matches!(phase, DecayPhase::Full | DecayPhase::Summary)
}

impl QueryEngine {
    pub(crate) fn supports_prepared_get(&self) -> bool {
        self.coherent_owner.is_some()
    }

    pub(crate) async fn get_contract(
        &self,
        request: crate::serialization::get::GetRequest,
        response_limit: usize,
    ) -> crate::serialization::get::GetReply {
        self.get_point_owned(request, response_limit, None).await
    }

    pub(crate) async fn get_api1_point(
        &self,
        request: crate::serialization::get_api1::Request,
        response_limit: usize,
        timezone: chrono_tz::Tz,
        #[cfg(test)] before_barrier: Option<fn(&crate::derived_completion::DerivedCompletion)>,
    ) -> crate::serialization::get::GetReply {
        let point = request.point();
        #[cfg(test)]
        let point = {
            let mut point = point;
            point.before_barrier = before_barrier;
            point
        };
        self.get_point_owned(point, response_limit, Some(timezone))
            .await
    }

    async fn get_point_owned(
        &self,
        request: crate::serialization::get::GetRequest,
        response_limit: usize,
        api1_timezone: Option<chrono_tz::Tz>,
    ) -> crate::serialization::get::GetReply {
        use crate::serialization::contract::{CommitState, Scope};
        use crate::serialization::get::{GetData, GetReply};
        if api1_timezone.is_some() {
            // API1 accepts syntactically valid nil as a real admitted missing
            // lookup. Do not widen the separate prepared/public2 validator.
            if let Some(name) = &request.namespace
                && crate::model::validation::validate_namespace_name(name).is_err()
            {
                return GetReply::input(crate::serialization::contract::InputError {
                    code: "OUT_OF_RANGE",
                    field: "/namespace".into(),
                    message: "invalid namespace assertion",
                });
            }
        } else if let Err(error) = request.validate() {
            return GetReply::input(error);
        }
        let fits = |reply: &GetReply, observe| match api1_timezone {
            Some(timezone) => crate::serialization::get_api1::fits(
                reply,
                timezone,
                crate::serialization::get_api1::Mode::Native,
                response_limit,
                observe,
            ),
            None => reply.fits(response_limit, observe),
        };
        let Some((owner, _)) = &self.coherent_owner else {
            return GetReply::failure(SearchError::Unavailable(
                "prepared get owner unavailable".into(),
            ));
        };
        let work = match owner.accept_work(None) {
            Ok(work) => work,
            Err(error) => return GetReply::failure(read_error(error)),
        };
        let mut captured = None;
        let result = async {
            let (mut selected, graph) = tokio::time::timeout(
                std::time::Duration::from_secs(30),
                self.capture_point_get(
                    request.id,
                    request.namespace.as_deref(),
                    request.record_access,
                    &work,
                    #[cfg(test)]
                    request.copy_pause.clone(),
                ),
            )
            .await
            .map_err(|_| SearchError::Unavailable("point capture deadline expired".into()))??;
            let scope = selected.payload.as_ref().map(|(ns, _)| Scope::Namespace {
                namespace_id: ns.id,
                namespace: ns.name.clone(),
            });
            captured = Some((scope.clone(), selected.at));
            let response =
                self.project_point_get(&mut selected, graph.as_ref(), request.record_access)?;
            // Actual publication acquires graph.write. Never carry the read
            // lease across exposure/completion, including behind a queued writer.
            drop(graph);
            let response = response.ok_or(SearchError::MemoryNotFound(request.id))?;
            let will_observe = request.record_access && exposed_phase(response.phase);
            let mut reply = GetReply::success(
                scope.expect("present source scope"),
                selected.at,
                GetData {
                    evaluated_at: selected.evaluated_at,
                    record_access: request.record_access,
                    memory: response,
                },
            );
            if !fits(&reply, will_observe) {
                return Ok(reply.oversize());
            }
            #[cfg(test)]
            if let Some((entered, release)) = &request.before_observation {
                entered.notify_one();
                release.notified().await;
            }
            reply.observation_receipts = self
                .observe_point_get(
                    &mut selected,
                    request.record_access,
                    &work,
                    request.original_contract_version,
                )
                .await?;
            reply.envelope.receipt = (reply.observation_receipts.len() == 1)
                .then(|| reply.observation_receipts[0].clone());
            Ok::<_, SearchError>(reply)
        }
        .await;
        let mut reply = match result {
            Ok(reply) => reply,
            Err(error) => GetReply::failure(error),
        };
        if reply.envelope.read_generation.is_none()
            && let Some((scope, at)) = captured
        {
            reply.envelope.scope = scope;
            reply.envelope.read_generation = Some(at);
        }
        if reply
            .envelope
            .error
            .as_ref()
            .is_some_and(|e| e.commit_state == CommitState::Unknown)
        {
            drop(work);
            return reply;
        }
        // Finish our root before a barrier that drains accepted descendants.
        owner.finish_known_work(work);
        #[cfg(test)]
        if let Some(hook) = request.before_barrier {
            hook(owner);
        }
        if reply.http_status == 200
            && let Some(receipt) = reply.observation_receipts.last()
        {
            let target = GenerationRef {
                store_id: receipt.store_id,
                generation: receipt.committed_generation,
            };
            match owner
                .readiness_barrier(
                    target,
                    std::time::Instant::now() + std::time::Duration::from_secs(30),
                )
                .await
            {
                Ok(readiness) => reply.envelope.readiness = Some(readiness.into()),
                Err(error) => {
                    let mut failed = GetReply::failure(SearchError::Observation(
                        crate::decay::observation::ObservationError::not_ready(
                            reply.observation_receipts,
                            error,
                        ),
                    ));
                    failed.envelope.scope = reply.envelope.scope;
                    failed.envelope.read_generation = reply.envelope.read_generation;
                    return failed;
                }
            }
        }
        if reply.http_status == 200 && !fits(&reply, false) {
            return reply.oversize();
        }
        reply
    }

    /// No std storage/publication guard crosses an await. Only an explicit
    /// lifecycle payload needs the graph; terminal identity needs no projection.
    async fn capture_point_get(
        &self,
        id: MemoryId,
        namespace: Option<&str>,
        record_access: bool,
        work: &WorkTicket,
        #[cfg(test)] copy_pause: Option<Arc<crate::serialization::get::PointCopyPause>>,
    ) -> Result<(PointSelection, Option<GraphLease>)> {
        let (owner, _) = self.coherent_owner.as_ref().ok_or_else(|| {
            SearchError::Unavailable("prepared get requires the actual publication owner".into())
        })?;
        let storage = owner.storage.clone();
        let child = work.child().map_err(|error| read_error(error.into()))?;
        let gate = tokio::task::spawn_blocking(move || {
            let storage = storage
                .read()
                .map_err(|e| SearchError::Unavailable(e.to_string()))?;
            let gate = storage.meta_store().derived_publication.clone();
            storage.meta_store().finish_work(child);
            Ok::<_, SearchError>(gate)
        })
        .await
        .map_err(|e| SearchError::Unavailable(e.to_string()))??;
        let publication = Arc::new(gate.read_owned().await);
        let first = self
            .copy_point_get(
                id,
                namespace,
                record_access,
                work,
                publication.clone(),
                None,
                None,
                #[cfg(test)]
                copy_pause.clone(),
            )
            .await?;
        match first {
            PointAttempt::Captured(selected) => Ok((selected, None)),
            PointAttempt::NeedsGraph(at) => {
                // The first attempt sampled no clock and captured no exposure.
                // Re-open under the real graph lease and reject a changed G.
                let graph = Arc::new(owner.graph.clone().read_owned().await);
                match self
                    .copy_point_get(
                        id,
                        namespace,
                        record_access,
                        work,
                        publication,
                        Some(graph.clone()),
                        Some(at),
                        #[cfg(test)]
                        copy_pause,
                    )
                    .await?
                {
                    PointAttempt::Captured(selected) => Ok((selected, Some(graph))),
                    PointAttempt::NeedsGraph(_) => Err(SearchError::Unavailable(
                        "point projection did not retain its graph lease".into(),
                    )),
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn copy_point_get(
        &self,
        id: MemoryId,
        namespace: Option<&str>,
        record_access: bool,
        work: &WorkTicket,
        publication: Arc<tokio::sync::OwnedRwLockReadGuard<()>>,
        graph: Option<GraphLease>,
        expected: Option<GenerationRef>,
        #[cfg(test)] copy_pause: Option<Arc<crate::serialization::get::PointCopyPause>>,
    ) -> Result<PointAttempt> {
        let (owner, _) = self.coherent_owner.as_ref().expect("checked point owner");
        let storage = owner.storage.clone();
        let namespace = namespace.map(str::to_owned);
        let policy = self.native_policy_inputs();
        let clock = self.clock.clone();
        let child = work.child().map_err(|error| read_error(error.into()))?;
        tokio::task::spawn_blocking(move || {
            // Retain both asynchronous leases if the awaiting caller cancels.
            let _publication = publication;
            let storage = storage
                .read()
                .map_err(|e| SearchError::Unavailable(e.to_string()))?;
            let result = storage
                .with_point_get(id, namespace.as_deref(), |point, payload| {
                    let at = point.view().at;
                    if expected.is_some_and(|expected| expected != at) {
                        return Err(StorageError::RecoveryRequired(
                            "canonical state changed while acquiring point projection inputs"
                                .into(),
                        ));
                    }
                    // A legacy-access point (#239) takes the connection bonus only
                    // while the graph view is ready; otherwise it reports the
                    // bonus-free projection rather than failing.
                    let projects = payload.as_ref().is_some_and(|(ns, row)| {
                        exposed_phase(row.record.phase)
                            && ns.schema.as_ref().is_some_and(|s| {
                                s.lifecycle_version
                                    == crate::model::schema::LifecycleVersion::ExplicitFeedbackV2
                                    || (point.require_graph().is_ok()
                                        && !matches!(
                                            super::strength::projection_policy(
                                                ns,
                                                &policy.global,
                                                policy.multiplier
                                            ),
                                            Ok(None)
                                        ))
                            })
                    });
                    if projects && graph.is_none() {
                        return Ok(PointAttempt::NeedsGraph(at));
                    }
                    let donors = if projects {
                        point.require_graph()?;
                        point
                            .view()
                            .records(payload.as_ref().expect("projected row").0.id)?
                    } else {
                        Vec::new()
                    };
                    let intent = if point.view().is_native_owned() && record_access {
                        payload
                            .as_ref()
                            .filter(|(_, row)| exposed_phase(row.record.phase))
                            .map(|(ns, _)| {
                                point.capture_exposure(
                                    LifecycleScope {
                                        namespace_id: ns.id,
                                        namespace_name: ns.name.clone(),
                                        source: Some(id),
                                    },
                                    id,
                                    &policy,
                                )
                            })
                            .transpose()?
                    } else {
                        None
                    };
                    let selected = PointSelection {
                        at,
                        evaluated_at: clock.now_millis(),
                        payload,
                        donors,
                        intent,
                    };
                    #[cfg(test)]
                    if let Some(pause) = copy_pause {
                        pause.pause()?;
                    }
                    Ok(PointAttempt::Captured(selected))
                })
                .map_err(read_error);
            storage.meta_store().finish_work(child);
            result
        })
        .await
        .map_err(|e| SearchError::Unavailable(e.to_string()))?
    }

    fn project_point_get(
        &self,
        selected: &mut PointSelection,
        graph: Option<&GraphLease>,
        record_access: bool,
    ) -> Result<Option<MemoryResponse>> {
        let Some((namespace, payload)) = &selected.payload else {
            return Ok(None);
        };
        let mut record = CachedRecord::from(&payload.record);
        if exposed_phase(record.phase) {
            let mut strength =
                StrengthContext::pin_at(self, namespace, record_access, selected.evaluated_at)?;
            match graph {
                Some(graph) if strength.projects() => {
                    let reader = SharedGraphReader::pinned(graph.clone());
                    strength.project_namespace(&mut selected.donors, &reader)?;
                }
                None if strength.explicit() => {
                    return Err(SearchError::Unavailable(
                        "missing graph lease for point projection".into(),
                    ));
                }
                // Legacy access without a ready graph view: no bonus (#239).
                _ => {}
            }
            strength.apply(&mut record)?;
        }
        Ok(Some(MemoryResponse {
            labels: record.labels.clone(),
            id: record.id,
            namespace: namespace.name.clone(),
            created_at: record.created_at,
            last_accessed_at: record.last_accessed_at,
            summary: exposed_phase(record.phase).then(|| record.summary.clone()),
            full_text: payload.full_text.clone(),
            tags: record.tags.clone(),
            phase: record.phase,
            strength: record.strength,
            decay_strength: record.decay_strength,
            stability: record.stability,
            is_permastore: record.is_permastore,
            edge_count: record.edge_count,
            access_count: payload.record.access_history.len(),
        }))
    }

    async fn observe_point_get(
        &self,
        selected: &mut PointSelection,
        record_access: bool,
        work: &WorkTicket,
        original_contract_version: u32,
    ) -> Result<Vec<crate::storage::control::WriteReceipt>> {
        let Some((namespace, payload)) = &selected.payload else {
            return Ok(Vec::new());
        };
        if !record_access || !exposed_phase(payload.record.phase) {
            return Ok(Vec::new());
        }
        let policy: NativePolicyInputs = self.native_policy_inputs();
        let id = MemoryId::from_bytes(payload.record.id);
        let observation = self
            .access_recorder
            .record_point_for_publication(
                id,
                &crate::storage::control::ExposureScope {
                    namespace: namespace.name.clone(),
                    source: Some(id),
                },
                selected.intent.take(),
                selected.evaluated_at,
                Some(work),
                &policy,
                original_contract_version,
            )
            .await?;
        let observed = Observed {
            value: (),
            receipts: observation.receipts,
        }
        .publish_derived(
            self.coherent_owner.as_ref().map(|(owner, _)| owner),
            Some(work),
        )
        .await?;
        Ok(observed.receipts)
    }
}
