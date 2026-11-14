// Included in read_view::tests to reuse the actual owned-engine fixture.
fn fixed_id(value: u128) -> MemoryId {
    MemoryId::from_uuid(uuid::Uuid::from_u128(
        0x00000000006470008000000000000000 | value,
    ))
}

async fn payload_record(
    owner: &DerivedCompletion,
    number: u128,
    embedding: [f32; 4],
    parent_id: Option<MemoryId>,
    supersedes: Option<MemoryId>,
    allowed: bool,
) -> MemoryId {
    let id = fixed_id(number);
    let inserted = owner
        .insert(
            MutationContext::unkeyed(NamespaceId::new(1)),
            InsertMemory {
                id: Some(id),
                summary: format!("payload {number}"),
                full_text: Some(format!("full payload {number}")),
                tags: if allowed {
                    vec![crate::model::Tag::new("allow").unwrap()]
                } else {
                    vec![]
                },
                initial_stability: None,
                created_at: Some(100),
                embedding: Some(embedding.to_vec()),
                parent_id,
                supersedes,
            },
            Vec::new().into(),
            100,
        )
        .await
        .unwrap();
    assert_eq!(
        crate::storage::insertion::inserted_id(&inserted.receipt).unwrap(),
        id
    );
    id
}

fn payload_query(depth: u8) -> SearchQuery {
    serde_json::from_value(serde_json::json!({
        "queryEmbedding":[1.0,0.0,0.0,0.0], "recordAccess":false,
        "graphDepth":depth, "limit":10, "minScore":0.9,
        "queryMode":"embedding_only"
    }))
    .unwrap()
}

fn payload_engine(f: &Fixture, owner: &DerivedCompletion) -> QueryEngine {
    let rif = Arc::new(crate::rif::RifEngine::new(Default::default()));
    engine(
        f,
        Arc::new(Embeddings {
            entered: Notify::new(),
            resume: Notify::new(),
        }),
        rif.clone(),
    )
    .with_coherent_owner(owner.clone(), rif)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn coherent_v2_ranked_terminal_does_not_fabricate_adjacent_relation() {
    let f = Fixture::new();
    let owner = owner(&f);
    let anchor = payload_record(&owner, 1, [1., 0., 0., 0.], None, None, true).await;
    let old = payload_record(&owner, 2, [0., 1., 0., 0.], Some(anchor), None, true).await;
    let terminal = payload_record(&owner, 3, [0., 0., 1., 0.], None, Some(old), true).await;
    let engine = payload_engine(&f, &owner);
    f.storage.write().unwrap().sync().unwrap();
    let before = bytes(&f);
    let current = engine
        .prepare_coherent_payload_with_semantics(
            payload_query(1),
            RetrievalSemantics::V2(VersionMode::Current),
            false,
            true,
            None,
        )
        .await
        .unwrap()
        .unwrap();
    let ranked: Vec<_> = current
        .response
        .response
        .results
        .iter()
        .map(|r| r.memory_id)
        .collect();
    assert!(ranked.contains(&anchor));
    assert!(ranked.contains(&terminal));
    assert!(!ranked.contains(&old));
    assert_eq!(
        current.response.diagnostics.graph_depths.get(&terminal),
        Some(&1)
    );
    let evidence = &current.response.diagnostics.provenance.as_ref().unwrap()[&terminal];
    assert!(
        evidence
            .inherited_channels
            .contains(&RetrievalChannel::Graph)
    );
    assert!(evidence.inherited_floor.is_some());
    let payload = current.payload.unwrap();
    assert_eq!(
        payload
            .rows
            .iter()
            .map(|r| MemoryId::from_bytes(r.record.id))
            .collect::<Vec<_>>(),
        ranked
    );
    assert!(payload.related.values().all(Vec::is_empty));
    assert!(payload.neighbors.is_empty());
    let coverage = payload.supplemental.unwrap();
    assert_eq!(coverage.historical_neighbors_omitted, 1);
    assert!(coverage.unresolved_neighbors.is_empty());
    assert!(coverage.versions.is_empty());
    let history = engine
        .prepare_coherent_payload_with_semantics(
            payload_query(1),
            RetrievalSemantics::V2(VersionMode::History),
            false,
            true,
            None,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        history.response.diagnostics.versions.get(&old),
        Some(&VersionStatus::Historical)
    );
    let payload = history.payload.unwrap();
    assert!(payload.related[&anchor].iter().any(|r| r.id == old));
    assert!(!payload.related[&anchor].iter().any(|r| r.id == terminal));
    assert!(
        !payload
            .neighbors
            .iter()
            .any(|n| MemoryId::from_bytes(n.payload.record.id) == terminal)
    );
    // The max-depth historical result cannot add an extra adjacency hop.
    assert!(payload.supplemental.unwrap().depth_limited_sources > 0);
    assert_eq!(bytes(&f), before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn coherent_v2_neighbor_cap_and_text_allowance_follow_hard_eligibility() {
    let f = Fixture::new();
    let owner = owner(&f);
    let anchor = payload_record(&owner, 100, [1., 0., 0., 0.], None, None, true).await;
    // UUID order deliberately puts twelve filtered neighbors ahead of survivors.
    for number in (1..=24).rev() {
        payload_record(
            &owner,
            number,
            [0., 1., 0., 0.],
            Some(anchor),
            None,
            number > 12,
        )
        .await;
    }
    let engine = payload_engine(&f, &owner);
    let mut query = payload_query(1);
    query.limit = 1;
    query.filter.require_tags = vec![crate::model::Tag::new("allow").unwrap()];
    f.storage.write().unwrap().sync().unwrap();
    let before = bytes(&f);
    let result = engine
        .prepare_coherent_payload_with_semantics(
            query.clone(),
            RetrievalSemantics::V2(VersionMode::Current),
            false,
            true,
            None,
        )
        .await
        .unwrap()
        .unwrap();
    let payload = result.payload.unwrap();
    assert_eq!(payload.rows.len(), 1);
    assert_eq!(MemoryId::from_bytes(payload.rows[0].record.id), anchor);
    let ids: Vec<_> = payload
        .neighbors
        .iter()
        .map(|n| MemoryId::from_bytes(n.payload.record.id))
        .collect();
    assert_eq!(ids, (13..=22).map(fixed_id).collect::<Vec<_>>());
    assert_eq!(
        payload
            .neighbors
            .iter()
            .map(|n| n.payload.full_text.is_some())
            .collect::<Vec<_>>(),
        vec![
            true, true, true, true, true, false, false, false, false, false
        ]
    );
    let coverage = payload.supplemental.unwrap();
    assert_eq!(coverage.ineligible_neighbors, 12);
    assert_eq!(coverage.eligible_unique_neighbors, 12);
    assert!(coverage.neighbor_limit_applied);
    assert_eq!(coverage.versions.keys().copied().collect::<Vec<_>>(), ids);
    assert!(coverage.depths.values().all(|depth| *depth == 1));
    query.graph_depth = 0;
    let zero = engine
        .prepare_coherent_payload_with_semantics(
            query.clone(),
            RetrievalSemantics::V2(VersionMode::Current),
            false,
            true,
            None,
        )
        .await
        .unwrap()
        .unwrap();
    assert!(zero.payload.unwrap().neighbors.is_empty());
    // Explicitly preserve the named V1 auxiliary behavior at depth zero.
    let legacy = engine
        .prepare_coherent_payload(query, false, true, None)
        .await
        .unwrap()
        .unwrap();
    assert!(legacy.payload.as_ref().unwrap().supplemental.is_none());
    assert_eq!(legacy.payload.unwrap().neighbors.len(), 10);
    assert_eq!(bytes(&f), before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn coherent_v2_selected_relations_do_not_spend_an_extra_hop() {
    let f = Fixture::new();
    let owner = owner(&f);
    let a = payload_record(&owner, 1, [1., 0., 0., 0.], None, None, true).await;
    let b = payload_record(&owner, 2, [1., 0., 0., 0.], Some(a), None, true).await;
    let engine = payload_engine(&f, &owner);
    let result = engine
        .prepare_coherent_payload_with_semantics(
            payload_query(0),
            RetrievalSemantics::V2(VersionMode::Current),
            false,
            true,
            None,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.response.diagnostics.graph_depths.get(&a), Some(&0));
    assert_eq!(result.response.diagnostics.graph_depths.get(&b), Some(&0));
    let payload = result.payload.unwrap();
    assert_eq!(payload.rows.len(), 2);
    assert!(payload.neighbors.is_empty());
    assert_eq!(payload.related[&a].len(), 1);
    assert_eq!(payload.related[&a][0].id, b);
    assert_eq!(payload.related[&b].len(), 1);
    assert_eq!(payload.related[&b][0].id, a);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn coherent_v2_missing_selected_graph_node_fails_payload_closed() {
    let f = Fixture::new();
    let owner = owner(&f);
    payload_record(&owner, 1, [1., 0., 0., 0.], None, None, true).await;
    let engine = payload_engine(&f, &owner);
    f.storage.write().unwrap().sync().unwrap();
    let before = bytes(&f);
    // Fault injection: corrupt a derived view while leaving its stamp in place.
    // History ranking can truthfully retain an independently eligible row with
    // unresolved version status, but cannot invent its missing adjacency.
    *f.graph.write().await = crate::graph::RelationshipGraph::new();
    let result = engine
        .prepare_coherent_payload_with_semantics(
            payload_query(0),
            RetrievalSemantics::V2(VersionMode::History),
            false,
            true,
            None,
        )
        .await;
    assert!(matches!(result, Err(SearchError::Unavailable(message))
        if message.contains("missing selected graph node")));
    assert_eq!(bytes(&f), before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn coherent_v2_filtered_terminal_is_known_historical_not_unresolved_neighbor() {
    let f = Fixture::new();
    let owner = owner(&f);
    let a = payload_record(&owner, 1, [1., 0., 0., 0.], None, None, true).await;
    let old = payload_record(&owner, 2, [0., 1., 0., 0.], Some(a), None, true).await;
    payload_record(&owner, 3, [0., 0., 1., 0.], None, Some(old), false).await;
    let engine = payload_engine(&f, &owner);
    let mut query = payload_query(1);
    query.filter.require_tags = vec![crate::model::Tag::new("allow").unwrap()];
    let result = engine
        .prepare_coherent_payload_with_semantics(
            query,
            RetrievalSemantics::V2(VersionMode::Current),
            false,
            true,
            None,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        result
            .response
            .response
            .results
            .iter()
            .map(|r| r.memory_id)
            .collect::<Vec<_>>(),
        vec![a]
    );
    let payload = result.payload.unwrap();
    assert!(payload.neighbors.is_empty());
    let coverage = payload.supplemental.unwrap();
    assert_eq!(coverage.historical_neighbors_omitted, 1);
    assert!(coverage.unresolved_neighbors.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn coherent_v2_supplemental_shared_budget_boundaries_are_truthful() {
    use super::super::semantics::MAX_SUPERSESSION_EDGES;
    use crate::graph::query::{MAX_GRAPH_EDGES, MAX_GRAPH_STATES};
    let f = Fixture::new();
    let owner = owner(&f);
    let a = payload_record(&owner, 1, [1., 0., 0., 0.], None, None, true).await;
    payload_record(&owner, 2, [0., 1., 0., 0.], Some(a), None, true).await;
    payload_record(&owner, 3, [0., 1., 0., 0.], Some(a), None, true).await;
    let engine = payload_engine(&f, &owner);
    let work = owner.accept_work(None).unwrap();
    let rif = Arc::new(crate::rif::RifEngine::new(Default::default()));
    let (pinned, canonical, graph, publication, _) =
        engine.pin_read_engine(&owner, rif, &work).await.unwrap();
    let (_, selection) = pinned
        .search_checked_with_selection(
            payload_query(1),
            RetrievalSemantics::V2(VersionMode::Current),
            true,
            None,
        )
        .await
        .unwrap();
    // Boundary injection applies to actual retained request context and graph;
    // it does not pretend these tiny fixtures themselves used 12,799 edges.
    let mut diagnostics = SemanticsDiagnostics::new(RetrievalSemantics::V2(VersionMode::Current));
    diagnostics.graph_depths.insert(a, 0);
    diagnostics.graph.examined_edges = MAX_GRAPH_EDGES - 1;
    let limited = pinned
        .select_payload_v2(
            &graph,
            &selection,
            &[a],
            true,
            VersionMode::Current,
            &mut diagnostics,
        )
        .await
        .unwrap();
    assert!(limited.neighbors.is_empty());
    assert_eq!(limited.diagnostics.edges_examined, 0);
    assert_eq!(limited.diagnostics.neighborhoods_skipped_edge_budget, 1);
    assert_eq!(diagnostics.graph.examined_edges, MAX_GRAPH_EDGES - 1);
    assert!(diagnostics.graph.edge_limit);
    diagnostics.graph.expanded_states = MAX_GRAPH_STATES;
    let limited = pinned
        .select_payload_v2(
            &graph,
            &selection,
            &[a],
            true,
            VersionMode::Current,
            &mut diagnostics,
        )
        .await
        .unwrap();
    assert_eq!(limited.diagnostics.neighborhoods_unexamined_state_budget, 1);
    assert!(diagnostics.graph.state_limit);
    diagnostics.graph = Default::default();
    diagnostics.supersession_edges_examined = MAX_SUPERSESSION_EDGES;
    let limited = pinned
        .select_payload_v2(
            &graph,
            &selection,
            &[a],
            true,
            VersionMode::Current,
            &mut diagnostics,
        )
        .await
        .unwrap();
    assert_eq!(
        limited
            .diagnostics
            .unresolved_neighbors
            .get(&UnresolvedReason::WorkLimit),
        Some(&2)
    );
    assert!(limited.diagnostics.supersession_work_limited);
    assert_eq!(limited.diagnostics.supersession_edges_examined, 0);
    assert_eq!(limited.diagnostics.historical_neighbors_omitted, 0);
    assert!(limited.neighbors.is_empty());
    assert_eq!(
        diagnostics.supersession_edges_examined,
        MAX_SUPERSESSION_EDGES
    );
    drop(pinned);
    drop(canonical);
    drop(graph);
    drop(publication);
    owner.finish_known_work(work);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn coherent_v2_final_hydrated_exposures_reach_actual_recorder_once() {
    use crate::search::adapters::StorageAccessRecorder;
    let f = Fixture::new();
    let owner = owner(&f);
    let a = payload_record(&owner, 1, [1., 0., 0., 0.], None, None, true).await;
    let old = payload_record(&owner, 2, [0., 1., 0., 0.], Some(a), None, true).await;
    let terminal = payload_record(&owner, 3, [0., 0., 1., 0.], None, Some(old), true).await;
    let mut engine = payload_engine(&f, &owner);
    engine.access_recorder = Arc::new(
        StorageAccessRecorder::new(f.storage.clone(), f.cache.clone())
            .with_clock(Arc::new(crate::time::ManualClock::new(999)))
            .with_completion(owner.clone()),
    );
    f.storage.write().unwrap().sync().unwrap();
    let before = bytes(&f);
    let before_a = f.record(a).unwrap();
    let before_terminal = f.record(terminal).unwrap();
    let before_old = f.record(old).unwrap().to_bytes();
    let readonly = engine
        .prepare_coherent_payload_with_semantics(
            payload_query(1),
            RetrievalSemantics::V2(VersionMode::Current),
            false,
            true,
            None,
        )
        .await
        .unwrap()
        .unwrap();
    let accesses = readonly.exposures(true).unwrap();
    assert_eq!(accesses.len(), 2);
    assert!(accesses.contains(&(a, AccessKind::DirectRetrieval)));
    assert!(accesses.contains(&(terminal, AccessKind::AssociativeRetrieval)));
    assert!(!accesses.iter().any(|(id, _)| *id == old));
    assert!(
        readonly
            .response
            .response
            .results
            .iter()
            .find(|r| r.memory_id == terminal)
            .unwrap()
            .activation_score
            .is_none()
    );
    assert_eq!(bytes(&f), before);
    let mut query = payload_query(1);
    query.record_access = true;
    let mut prepared = engine
        .prepare_coherent_payload_with_semantics(
            query.clone(),
            RetrievalSemantics::V2(VersionMode::Current),
            false,
            true,
            None,
        )
        .await
        .unwrap()
        .unwrap();
    // Retaining true intent during preparation still defers the actual write.
    assert_eq!(bytes(&f), before);
    assert_eq!(prepared.evaluated_at, 123);
    // An overlapping final presentation cannot downgrade direct access or
    // create two events. The duplicated owned row is actual captured content.
    let payload = prepared.payload.as_mut().unwrap();
    let duplicate = payload
        .rows
        .iter()
        .find(|r| r.record.id == *a.as_bytes())
        .unwrap()
        .clone();
    payload.neighbors.push(CoherentNeighbor {
        payload: duplicate,
        edge_type: "parentchild".into(),
        weight: 1.,
        connected_to: terminal,
    });
    assert_eq!(prepared.exposures(true).unwrap(), accesses);
    assert!(prepared.exposures(false).is_err());
    let legacy = engine
        .prepare_coherent_payload(query, false, true, None)
        .await
        .unwrap()
        .unwrap();
    let legacy_accesses = legacy.exposures(true).unwrap();
    for result in &legacy.response.response.results {
        let kind = if result.activation_score.is_some() {
            AccessKind::AssociativeRetrieval
        } else {
            AccessKind::DirectRetrieval
        };
        assert!(legacy_accesses.contains(&(result.memory_id, kind)));
    }
    assert_eq!(bytes(&f), before);
    let receipts = engine
        .record_exposures_at_with_work(
            &prepared.exposures(true).unwrap(),
            crate::storage::control::ExposureScope {
                namespace: "default".into(),
                source: None,
            },
            prepared.evaluated_at,
            None,
        )
        .await
        .unwrap();
    assert!(!receipts.is_empty());
    let after_a = f.record(a).unwrap();
    let after_terminal = f.record(terminal).unwrap();
    assert_eq!(
        after_a.access_history.len(),
        before_a.access_history.len() + 1
    );
    assert_eq!(
        after_terminal.access_history.len(),
        before_terminal.access_history.len() + 1
    );
    assert_eq!(
        after_a.access_history.last().unwrap().kind,
        AccessKind::DirectRetrieval
    );
    assert_eq!(
        after_terminal.access_history.last().unwrap().kind,
        AccessKind::AssociativeRetrieval
    );
    assert_eq!(after_a.access_history.last().unwrap().timestamp, 123);
    assert_eq!(after_terminal.access_history.last().unwrap().timestamp, 123);
    assert_eq!(f.record(old).unwrap().to_bytes(), before_old);
    assert!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .publication_state()
            .unwrap()
            .matches_canonical()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn coherent_v2_payload_ties_ignore_adjacency_insertion_order() {
    use crate::model::EdgeType;
    let f = Fixture::new();
    let owner = owner(&f);
    let a = payload_record(&owner, 1, [1., 0., 0., 0.], None, None, true).await;
    let b = payload_record(&owner, 2, [1., 0., 0., 0.], None, None, true).await;
    let c = payload_record(&owner, 3, [0., 1., 0., 0.], None, None, true).await;
    let d = payload_record(&owner, 4, [0., 1., 0., 0.], None, None, true).await;
    for (source, target, kind) in [
        (b, c, EdgeType::Entity),
        (a, c, EdgeType::Temporal),
        (c, a, EdgeType::Entity),
        (a, c, EdgeType::Entity),
        (a, d, EdgeType::Entity),
        (a, b, EdgeType::Temporal),
        (a, b, EdgeType::Entity),
    ] {
        f.storage
            .read()
            .unwrap()
            .add_edge(source, target, kind, 0.5, false, 100)
            .unwrap();
    }
    owner.complete().await.unwrap();
    let engine = payload_engine(&f, &owner);
    let mut query = payload_query(1);
    query.limit = 2;
    let mut snapshots = Vec::new();
    for reverse in [false, true] {
        if reverse {
            // Same canonical edges and same G, different valid adjacency order.
            let mut graph = f.graph.write().await;
            for node in graph.nodes.values_mut() {
                node.outgoing.reverse();
                node.incoming.reverse();
            }
        }
        let prepared = engine
            .prepare_coherent_payload_with_semantics(
                query.clone(),
                RetrievalSemantics::V2(VersionMode::Current),
                false,
                true,
                None,
            )
            .await
            .unwrap()
            .unwrap();
        let payload = prepared.payload.unwrap();
        let neighbors: Vec<_> = payload
            .neighbors
            .iter()
            .map(|n| {
                (
                    MemoryId::from_bytes(n.payload.record.id),
                    n.weight.to_bits(),
                    n.edge_type.clone(),
                    n.connected_to,
                )
            })
            .collect();
        assert_eq!(
            neighbors,
            vec![
                (c, 0.5f32.to_bits(), "entity".into(), a),
                (d, 0.5f32.to_bits(), "entity".into(), a),
            ]
        );
        let related: Vec<_> = [a, b]
            .into_iter()
            .map(|id| {
                (
                    id,
                    payload.related[&id]
                        .iter()
                        .map(|r| (r.id, r.edge_type.clone(), r.weight.to_bits()))
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        assert_eq!(
            related[0]
                .1
                .iter()
                .map(|r| r.1.as_str())
                .collect::<Vec<_>>(),
            vec!["entity", "temporal"]
        );
        snapshots.push((
            serde_json::to_value(prepared.response.response.results).unwrap(),
            neighbors,
            related,
        ));
    }
    assert_eq!(snapshots[0], snapshots[1]);
}
