//! Typed single producer for the native wire consumer. Public legacy dispatch
//! is unchanged; a single operation never acquires catalog batch semantics.
use super::McpStorageAdapter;
use crate::storage::StorageEngine as _;
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
    pub(super) async fn run_native_single_wire(
        &self,
        request: crate::serialization::native_single::Request,
    ) -> crate::serialization::native_single::Reply {
        use crate::serialization::{contract::*, native_single::Reply};
        use crate::storage::metadata::NativeStoreProfileResolution;
        let mut parsed = match request.prepare() {
            Ok(v) => v,
            Err(e) => return Reply::invalid(e),
        };
        let completion = self.batch_completion();
        let work = match completion.accept_work(None) {
            Ok(work) => work,
            Err(e) => return Reply::failure(e),
        };
        let mut scope = None;
        let mut known = None;
        let result: Result<_, StorageError> = async {
            let ctx;
            {
                let engine = self
                    .storage
                    .read()
                    .map_err(|e| StorageError::RecoveryRequired(e.to_string()))?;
                // This immutable owned entry mode is known before any provider,
                // clock, reservation or native mutation. Do not misclassify an
                // unsupported legacy semantic request as an uncertain commit.
                // Native and legacy receipts have different original kinds;
                // this endpoint must never guess a legacy replay/fallback.
                if !engine.meta_store().uses_native_lifecycle() {
                    return Err(StorageError::UnsupportedMutation(
                        crate::storage::insertion::LEGACY_API2_SEMANTIC_UNSUPPORTED.into(),
                    ));
                }
                let namespace_id = match parsed.namespace_id {
                    Some(id) => id,
                    None => {
                        engine
                            .get_namespace_by_name(&parsed.store.namespace)?
                            .ok_or_else(|| {
                                StorageError::NamespaceNameNotFound(parsed.store.namespace.clone())
                            })?
                            .id
                    }
                };
                ctx = parsed.options.context(
                    crate::storage::control::MutationScope::Namespace(namespace_id),
                    ContractVersion::V2,
                );
                // Resolve matching history before current binding, policy, CAS,
                // settings, provider, clocks or payload allocation.
                let resolution = engine.meta_store().resolve_native_store_profile(
                    &ctx,
                    &parsed.input,
                    parsed.profile_id.as_deref(),
                    Some(&work),
                )?;
                if let NativeStoreProfileResolution::Replay(receipt) = &resolution {
                    known = Some(receipt.clone());
                }
                let guard = engine.meta_store().native_guard_with_work(Some(&work))?;
                let (generation, binding) = engine
                    .meta_store()
                    .capture_namespace_binding(&guard, namespace_id)?;
                let binding = binding.ok_or_else(|| {
                    if known.is_some() {
                        StorageError::RecoveryRequired(
                            "retained namespace binding unavailable".into(),
                        )
                    } else {
                        StorageError::NamespaceNotFound(namespace_id.get())
                    }
                })?;
                if parsed
                    .asserted_name
                    .as_ref()
                    .is_some_and(|name| !name.eq_ignore_ascii_case(&binding.name))
                {
                    return Err(StorageError::MutationNamespaceMismatch);
                }
                parsed.store.namespace = binding.name.clone();
                scope = Some((namespace_id, binding.name));
                if matches!(resolution, NativeStoreProfileResolution::Fresh(_)) {
                    if let Some(expected) = ctx.expected_generation {
                        if expected != generation.generation {
                            return Err(StorageError::GenerationConflict {
                                expected: String::from(expected),
                                actual: String::from(generation.generation),
                            });
                        }
                    }
                    let settings =
                        crate::graph::native_selection::freeze_settings(&self.config.graph)?;
                    let bound = super::super::batch_budget::BatchBudget::receipt_bound(
                        &parsed.input,
                        &settings,
                    );
                    // Same typed receipt maximum as batches, plus single data,
                    // readiness and both MCP copies. Never change link settings.
                    if bound + 32 * 1024 > super::super::batch_budget::MAX_BATCH_JSON_BYTES {
                        return Err(StorageError::BatchResponseTooLarge {
                            reservation_receipt: None,
                        });
                    }
                }
            }
            let policy = NativePolicyInputs {
                global: crate::decay::DecayConfig::default(),
                multiplier: self.config.decay.decay_rate_multiplier,
            };
            let result = self
                .store_native_single_by_profile_id(
                    ctx,
                    parsed.input,
                    parsed.profile_id,
                    policy,
                    Some(&work),
                )
                .await?;
            known = Some(result.receipt.clone());
            let mut data = super::super::batches::stored_from_receipt(
                &parsed.store,
                &result.receipt,
                self.timezone,
            )
            .map_err(|e| StorageError::RecoveryRequired(e.to_string()))?;
            data.near_duplicates = result.advisory.report;
            Ok((
                result.receipt,
                data,
                result.advisory.warnings,
                result.advisory.read_generation,
            ))
        }
        .await;
        // Finish only the known root; worker tickets retain unknown commits.
        completion.finish_known_work(work);
        let deadline = || std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut reply = match result {
            Ok((receipt, data, warnings, read_generation)) => {
                let target = self
                    .storage
                    .read()
                    .map_err(|e| StorageError::RecoveryRequired(e.to_string()))
                    .and_then(|s| s.control_state().map(|c| c.generation_ref()));
                let ready = match target {
                    Ok(target) => completion.readiness_barrier(target, deadline()).await,
                    Err(e) => Err(e),
                };
                match ready {
                    Ok(ready) => Reply {
                        namespace_id: None,
                        namespace: None,
                        data: Some(data),
                        receipt: Some(receipt),
                        readiness: Some(ready.into()),
                        read_generation,
                        error: None,
                        warnings: warnings.into_iter().map(str::to_owned).collect(),
                    },
                    Err(source) => {
                        let mut reply = Reply::failure(StorageError::CommittedMutation {
                            receipt: Box::new(receipt),
                            source: Box::new(source),
                        });
                        reply.read_generation = read_generation;
                        reply
                            .warnings
                            .extend(warnings.into_iter().map(str::to_owned));
                        reply
                    }
                }
            }
            Err(error) => {
                let error = match (known, error) {
                    (_, error @ StorageError::CommittedMutation { .. }) => error,
                    (Some(receipt), source) => StorageError::CommittedMutation {
                        receipt: Box::new(receipt),
                        source: Box::new(source),
                    },
                    (_, error) => error,
                };
                Reply::failure(error)
            }
        };
        if let Some((id, name)) = scope {
            reply.namespace_id = Some(id);
            reply.namespace = Some(name);
        }
        if reply.error.as_ref().is_some_and(|error| {
            matches!(
                error.commit_state,
                CommitState::Committed | CommitState::Unknown
            )
        }) {
            match completion
                .observe_readiness(deadline())
                .await
                .and_then(Readiness::observed)
            {
                Ok(readiness) => reply.readiness = Some(readiness),
                Err(_) => reply.warnings.push("READINESS_UNKNOWN".into()),
            }
        }
        reply
    }
    /// Wire boundary: input carries original fields/vectors and no descriptor.
    /// A supplied vector requires its explicit ID; generated input omits both.
    pub(crate) async fn store_native_single_by_profile_id(
        &self,
        ctx: MutationContext,
        input: NativeStoreInput,
        supplied_profile_id: Option<String>,
        policy: NativePolicyInputs,
        parent: Option<&WorkTicket>,
    ) -> Result<NativeSingleResult, StorageError> {
        let completion = self.batch_completion();
        let work = completion.accept_work(parent)?;
        let result = async {
            let receipt = completion
                .store_native_profile_with_work(
                    ctx,
                    input,
                    supplied_profile_id,
                    self.embedding.clone(),
                    self.config.graph.clone(),
                    policy,
                    self.clock.clone(),
                    Some(&work),
                )
                .await?;
            self.native_single_result(receipt)
        }
        .await;
        completion.finish_known_work(work);
        result
    }

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
            self.native_single_result(receipt)
        }
        .await;
        completion.finish_known_work(work);
        result
    }
    fn native_single_result(
        &self,
        receipt: WriteReceipt,
    ) -> Result<NativeSingleResult, StorageError> {
        let MutationOutcome::NativeMemoryInsertedWithLinks { transition } = &receipt.outcome else {
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
}
