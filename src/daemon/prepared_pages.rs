//! Private canonical metadata transport. No observation, provider, or v1 fallback.
use crate::{
    canonical_list::{ListData, ListError, ListPage},
    catalog_page::{CatalogData, CatalogError, CatalogPage, ProfileCompatibility},
    mcp::bridge::{BridgeError, StorageEngine},
    model::{MemoryId, NamespaceId},
    serialization::{
        catalog_page::{CatalogCursor, CatalogKind, CatalogPageRequest},
        contract::{Envelope, Scope},
        list::{ListCursor, ListPageRequest},
    },
    storage::{canonical_vector_snapshot::SnapshotBinding, control::GenerationRef},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub(crate) const LIST: &str = "canonical_list_page_v1";
pub(crate) const CATALOG: &str = "canonical_catalog_page_v1";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Reply {
    wire_version: u32,
    result: Page,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum Page {
    List {
        at: GenerationRef,
        #[serde(rename = "namespaceId")]
        namespace_id: NamespaceId,
        #[serde(rename = "namespaceSha256")]
        namespace_sha256: [u8; 32],
        namespace: String,
        data: ListData,
    },
    Catalog {
        at: GenerationRef,
        scope: Scope,
        data: CatalogData,
    },
    Failure {
        error: Failure,
    },
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "camelCase", deny_unknown_fields)]
enum Failure {
    NotFound,
    Stale,
    TooLarge,
    CatalogTooLarge,
    InvalidCursor,
    LegacyFilter { field: String },
    Unavailable,
}
impl From<ListError> for Failure {
    fn from(error: ListError) -> Self {
        match error {
            ListError::NotFound => Self::NotFound,
            ListError::Stale => Self::Stale,
            ListError::TooLarge => Self::TooLarge,
            ListError::Input(e) if e.code == "INVALID_CURSOR" => Self::InvalidCursor,
            ListError::Input(e)
                if e.code == "OUT_OF_RANGE"
                    && matches!(e.field.as_str(), "/entities" | "/topics" | "/emotions") =>
            {
                Self::LegacyFilter { field: e.field }
            }
            _ => Self::Unavailable,
        }
    }
}
impl From<CatalogError> for Failure {
    fn from(error: CatalogError) -> Self {
        match error {
            CatalogError::NotFound => Self::NotFound,
            CatalogError::Stale => Self::Stale,
            CatalogError::ResponseTooLarge => Self::TooLarge,
            CatalogError::CatalogTooLarge => Self::CatalogTooLarge,
            CatalogError::Input(e) if e.code == "INVALID_CURSOR" => Self::InvalidCursor,
            _ => Self::Unavailable,
        }
    }
}
impl Failure {
    fn list(self) -> ListError {
        match self {
            Self::NotFound => ListError::NotFound,
            Self::Stale => ListError::Stale,
            Self::TooLarge => ListError::TooLarge,
            Self::InvalidCursor => ListCursor::invalid().into(),
            Self::LegacyFilter { field }
                if matches!(field.as_str(), "/entities" | "/topics" | "/emotions") =>
            {
                crate::serialization::list::invalid(
                    "OUT_OF_RANGE",
                    &field,
                    "structured filter cannot be encoded by this legacy namespace",
                )
                .into()
            }
            _ => ListError::Unavailable,
        }
    }
    fn catalog(self) -> CatalogError {
        match self {
            Self::NotFound => CatalogError::NotFound,
            Self::Stale => CatalogError::Stale,
            Self::TooLarge => CatalogError::ResponseTooLarge,
            Self::CatalogTooLarge => CatalogError::CatalogTooLarge,
            Self::InvalidCursor => CatalogCursor::invalid().into(),
            _ => CatalogError::Unavailable,
        }
    }
}

fn list_arguments(r: &ListPageRequest) -> Result<Value, ListError> {
    let mut v = json!({"contractVersion":2,"namespace":r.namespace,"limit":r.limit,
        "tags":r.filters.tags,"entities":r.filters.labels.entities,
        "topics":r.filters.labels.topics,"emotions":r.filters.labels.emotions});
    if let Some(c) = &r.cursor {
        v["cursor"] = json!(c);
    }
    if let Some(t) = r.filters.time_range_start {
        v["timeRangeStart"] = json!(t);
    }
    if let Some(t) = r.filters.time_range_end {
        v["timeRangeEnd"] = json!(t);
    }
    if let Some(p) = r.filters.phase {
        v["phase"] = json!(match p {
            crate::model::DecayPhase::Full => "full",
            crate::model::DecayPhase::Summary => "summary",
            crate::model::DecayPhase::Ghost => "ghost",
            crate::model::DecayPhase::Tombstone => "tombstone",
        });
    }
    ListPageRequest::from_mcp(&v)?;
    if r.namespace.is_none() {
        return Err(ListError::Unavailable);
    }
    Ok(v)
}
fn catalog_arguments(r: &CatalogPageRequest) -> Result<Value, CatalogError> {
    let mut v = json!({"contractVersion":2,"limit":r.limit});
    if let Some(c) = &r.cursor {
        v["cursor"] = json!(c);
    }
    if r.kind == CatalogKind::Tags {
        v["allNamespaces"] = json!(r.all_namespaces);
        v["namespace"] = json!(r.namespace);
    } else if !r.all_namespaces || r.namespace.is_some() {
        return Err(CatalogError::Unavailable);
    }
    CatalogPageRequest::from_mcp(&v, r.kind)?;
    if !r.all_namespaces && r.namespace.is_none() {
        return Err(CatalogError::Unavailable);
    }
    Ok(v)
}

async fn exchange(client: &super::DaemonClient, method: &str, params: Value) -> Option<Page> {
    let exchange = client.call_prepared(method, params).await;
    if exchange.phase != super::client::TransmissionPhase::MatchedReply {
        return None;
    }
    let value = exchange.result?;
    let reply: Reply = serde_json::from_value(value.clone()).ok()?;
    // Reject ignored fields, normalized tags, omitted required nullable members,
    // and noncanonical identities. These DTOs contain no floating-point values.
    if reply.wire_version != 1 || serde_json::to_value(&reply).ok()? != value {
        return None;
    }
    Some(reply.result)
}
pub(crate) async fn remote_list(
    client: &super::DaemonClient,
    request: ListPageRequest,
) -> Result<ListPage, ListError> {
    let arguments = list_arguments(&request)?;
    if let Some(c) = &request.cursor {
        ListCursor::decode(c, &request)?;
    }
    let params =
        json!({"arguments":arguments,"responseLimit":super::prepared_query::response_limit()});
    match exchange(client, LIST, params).await {
        Some(Page::List {
            at,
            namespace_id,
            namespace_sha256,
            namespace,
            data,
        }) => {
            let page = ListPage {
                binding: SnapshotBinding {
                    at,
                    namespace_id,
                    namespace_sha256,
                },
                namespace,
                data,
            };
            validate_list(&page, &request).ok_or(ListError::Unavailable)?;
            Ok(page)
        }
        Some(Page::Failure { error }) => Err(error.list()),
        _ => Err(ListError::Unavailable),
    }
}
pub(crate) async fn remote_catalog(
    client: &super::DaemonClient,
    request: CatalogPageRequest,
) -> Result<CatalogPage, CatalogError> {
    let arguments = catalog_arguments(&request)?;
    if let Some(c) = &request.cursor {
        CatalogCursor::decode(c, &request)?;
    }
    let params = json!({"arguments":arguments,"catalogKind":request.kind,
        "responseLimit":super::prepared_query::response_limit()});
    match exchange(client, CATALOG, params).await {
        Some(Page::Catalog { at, scope, data }) => {
            let page = CatalogPage { at, scope, data };
            validate_catalog(&page, &request).ok_or(CatalogError::Unavailable)?;
            Ok(page)
        }
        Some(Page::Failure { error }) => Err(error.catalog()),
        _ => Err(CatalogError::Unavailable),
    }
}

/// Count before allocating the private JSON tree. This is a serialized budget,
/// not an allocator/RSS bound. Whole-page failure preserves the caller cursor.
fn bounded_value(value: &impl Serialize) -> Option<Value> {
    struct Capped(Vec<u8>);
    impl std::io::Write for Capped {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() > super::prepared_query::response_limit().saturating_sub(self.0.len()) {
                return Err(std::io::Error::other("page limit"));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = Capped(Vec::new());
    serde_json::to_writer(&mut writer, value).ok()?;
    serde_json::from_slice(&writer.0).ok()
}
fn fits(value: &impl Serialize, limit: usize) -> bool {
    struct Count(usize, usize);
    impl std::io::Write for Count {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            if b.len() > self.1.saturating_sub(self.0) {
                return Err(std::io::Error::other("page limit"));
            }
            self.0 += b.len();
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(Count(0, limit), value).is_ok()
}
fn reply(result: Page) -> Result<Value, BridgeError> {
    let envelope = match &result {
        Page::List {
            at,
            namespace_id,
            namespace,
            data,
            ..
        } => {
            let mut e = Envelope::success(
                Scope::Namespace {
                    namespace_id: *namespace_id,
                    namespace: namespace.clone(),
                },
                data,
            );
            e.read_generation = Some(*at);
            bounded_value(&e)
        }
        Page::Catalog { at, scope, data } => {
            let mut e = Envelope::success(scope.clone(), data);
            e.read_generation = Some(*at);
            bounded_value(&e)
        }
        Page::Failure { .. } => Some(Value::Null),
    };
    let too_large =
        || json!({"wireVersion":1,"result":{"kind":"failure","error":{"code":"tooLarge"}}});
    let Some(envelope) = envelope else {
        return Ok(too_large());
    };
    if !envelope.is_null() {
        let tool = crate::mcp::protocol::ToolCallResult::json(&envelope)
            .map_err(|_| BridgeError::Internal("canonical page serialization".into()))?;
        // The actual inner modern MCP result is larger than the legacy result.
        if !fits(&tool, super::protocol::MAX_MESSAGE_SIZE) {
            return Ok(too_large());
        }
    }
    let Some(value) = bounded_value(&Reply {
        wire_version: 1,
        result,
    }) else {
        return Ok(too_large());
    };
    if !fits(
        &super::protocol::DaemonResponse::success(u64::MAX, value.clone()),
        super::prepared_query::response_limit(),
    ) {
        return Ok(too_large());
    }
    Ok(value)
}

pub(crate) async fn dispatch(
    storage: &dyn StorageEngine,
    method: &str,
    params: Value,
    peer_limit: Option<usize>,
) -> Result<Value, BridgeError> {
    if !storage.supports_canonical_pages()
        || peer_limit.is_none_or(|n| n < super::protocol::MAX_MESSAGE_SIZE)
    {
        return Err(BridgeError::Unavailable(
            "canonical pages require a capable negotiated full-frame daemon".into(),
        ));
    }
    let object = params
        .as_object()
        .ok_or_else(|| BridgeError::InvalidInput("page request must be object".into()))?;
    let expected = if method == LIST { 2 } else { 3 };
    if object.len() != expected
        || object.get("responseLimit").and_then(Value::as_u64)
            != Some(super::prepared_query::response_limit() as u64)
    {
        return Err(BridgeError::InvalidInput(
            "invalid page response allowance".into(),
        ));
    }
    let args = object
        .get("arguments")
        .ok_or_else(|| BridgeError::InvalidInput("missing page arguments".into()))?;
    let result = match method {
        LIST => {
            let r = ListPageRequest::from_mcp(args)
                .map_err(|e| BridgeError::InvalidInput(e.to_string()))?;
            list_arguments(&r).map_err(|e| BridgeError::InvalidInput(e.to_string()))?;
            match storage.list_canonical_page(r).await {
                Ok(p) => Page::List {
                    at: p.binding.at,
                    namespace_id: p.binding.namespace_id,
                    namespace_sha256: p.binding.namespace_sha256,
                    namespace: p.namespace,
                    data: p.data,
                },
                Err(e) => Page::Failure { error: e.into() },
            }
        }
        CATALOG => {
            let kind: CatalogKind =
                serde_json::from_value(object.get("catalogKind").cloned().unwrap_or(Value::Null))
                    .map_err(|_| BridgeError::InvalidInput("invalid catalog kind".into()))?;
            let r = CatalogPageRequest::from_mcp(args, kind)
                .map_err(|e| BridgeError::InvalidInput(e.to_string()))?;
            catalog_arguments(&r).map_err(|e| BridgeError::InvalidInput(e.to_string()))?;
            match storage.list_catalog_page(r).await {
                Ok(p) => {
                    let p = p.with_active_embedding(storage.catalog_profile_context());
                    Page::Catalog {
                        at: p.at,
                        scope: p.scope,
                        data: p.data,
                    }
                }
                Err(e) => Page::Failure { error: e.into() },
            }
        }
        _ => {
            return Err(BridgeError::InvalidInput(
                "unsupported canonical page method".into(),
            ));
        }
    };
    reply(result)
}

fn namespace(name: &str, expected: Option<&str>) -> bool {
    crate::model::validation::validate_namespace_name(name).is_ok()
        && expected.is_some_and(|e| name.eq_ignore_ascii_case(e))
}
fn valid_total(total: u64, returned: usize, has_prior: bool, has_more: bool) -> bool {
    // The opaque cursor proves at least one prior entry, not its ordinal.
    u64::try_from(returned)
        .ok()
        .and_then(|n| n.checked_add(u64::from(has_prior)))
        .and_then(|n| n.checked_add(u64::from(has_more)))
        .is_some_and(|minimum| total >= minimum)
}

fn validate_list(p: &ListPage, r: &ListPageRequest) -> Option<()> {
    let d = &p.data;
    if p.binding.at.store_id.is_nil()
        || p.binding.namespace_id.is_unset()
        || !namespace(&p.namespace, r.namespace.as_deref())
        || d.limit != r.limit
        || d.memories.len() > r.limit
        || !valid_total(
            d.total.get(),
            d.memories.len(),
            r.cursor.is_some(),
            !d.complete,
        )
        || d.complete != d.next_cursor.is_none()
        || (!d.complete && d.memories.len() != r.limit)
    {
        return None;
    }
    let incoming = r
        .cursor
        .as_deref()
        .map(|c| ListCursor::decode(c, r))
        .transpose()
        .ok()?;
    if let Some(c) = &incoming {
        if c.at != p.binding.at
            || c.namespace_id != p.binding.namespace_id
            || c.namespace_sha256 != p.binding.namespace_sha256
        {
            return None;
        }
    } else if d.complete && d.total.get() != d.memories.len() as u64 {
        return None;
    }
    let mut previous = incoming.as_ref().map(|c| c.after);
    let labels = r.filters.labels.compile();
    for entry in &d.memories {
        if entry.id.into_inner().is_nil()
            || previous.is_some_and(|id: MemoryId| id.as_bytes() >= entry.id.as_bytes())
            || entry.phase == crate::model::DecayPhase::Tombstone
            || r.filters.phase.is_some_and(|p| p != entry.phase)
            || r.filters
                .time_range_start
                .is_some_and(|t| entry.created_at < t)
            || r.filters
                .time_range_end
                .is_some_and(|t| entry.created_at > t)
            || !r.filters.tags.iter().all(|t| entry.tags.contains(t))
            || !labels.matches(&entry.tags, entry.labels.as_ref())
        {
            return None;
        }
        let (entities, topics, emotions) = if let Some(l) = &entry.labels {
            l.validate().ok()?;
            (&l.entities, &l.topics, &l.emotions)
        } else {
            let l = crate::model::tag::parse_structured_tags(&entry.tags);
            if l.entities != entry.entities
                || l.topics != entry.topics
                || l.emotions != entry.emotions
            {
                return None;
            }
            (&entry.entities, &entry.topics, &entry.emotions)
        };
        if entities != &entry.entities || topics != &entry.topics || emotions != &entry.emotions {
            return None;
        }
        previous = Some(entry.id);
    }
    if let Some(token) = &d.next_cursor {
        let c = ListCursor::decode(token, r).ok()?;
        if c.at != p.binding.at
            || c.namespace_id != p.binding.namespace_id
            || c.namespace_sha256 != p.binding.namespace_sha256
            || Some(c.after) != previous
        {
            return None;
        }
    }
    Some(())
}

fn validate_catalog(p: &CatalogPage, r: &CatalogPageRequest) -> Option<()> {
    if p.at.store_id.is_nil() {
        return None;
    }
    let selected = match (&p.scope, r.all_namespaces) {
        (Scope::Catalog { .. }, true) => None,
        (
            Scope::Namespace {
                namespace_id,
                namespace: name,
            },
            false,
        ) if !namespace_id.is_unset() && namespace(name, r.namespace.as_deref()) => {
            Some(*namespace_id)
        }
        _ => return None,
    };
    let incoming = r
        .cursor
        .as_deref()
        .map(|c| CatalogCursor::decode(c, r))
        .transpose()
        .ok()?;
    if incoming
        .as_ref()
        .is_some_and(|c| c.at != p.at || c.namespace_id != selected)
    {
        return None;
    }
    let (limit, complete, token) = match (&p.data, r.kind) {
        (CatalogData::Namespaces(d), CatalogKind::Namespaces) => {
            if d.namespaces.len() > r.limit
                || !valid_total(
                    d.total.get(),
                    d.namespaces.len(),
                    r.cursor.is_some(),
                    !d.complete,
                )
                || (!d.complete && d.namespaces.len() != r.limit)
            {
                return None;
            }
            if incoming.is_none() && d.complete && d.total.get() != d.namespaces.len() as u64 {
                return None;
            }
            if let Some(active) = &d.configured_embedding {
                crate::serialization::session::BackendContext::new(p.at.store_id, active.clone())
                    .ok()?;
            }
            let mut previous = incoming
                .as_ref()
                .and_then(|c| c.namespace_after)
                .unwrap_or(0);
            for e in &d.namespaces {
                if e.id.get() <= previous
                    || crate::model::validation::validate_namespace_name(&e.name).is_err()
                    || e.embedding_profile_id.is_some() != e.binding_provenance.is_some()
                {
                    return None;
                }
                if let Some(id) = &e.embedding_profile_id {
                    crate::serialization::session::BackendContext::new(
                        p.at.store_id,
                        crate::serialization::session::ActiveEmbedding::Configuration {
                            dimension: e.embedding_dim,
                            profile_id: id.clone(),
                        },
                    )
                    .ok()?;
                }
                // Validate BEFORE the outer MCP context finalizer can recompute.
                use crate::serialization::session::ActiveEmbedding;
                let expected = match (&e.embedding_profile_id, &d.configured_embedding) {
                    (None, _) => ProfileCompatibility::UnknownNamespaceProfile,
                    (_, None) => ProfileCompatibility::UnknownActiveProfile,
                    (
                        _,
                        Some(
                            ActiveEmbedding::Unknown { dimension }
                            | ActiveEmbedding::Configuration { dimension, .. },
                        ),
                    ) if *dimension != e.embedding_dim => ProfileCompatibility::DimensionMismatch,
                    (_, Some(ActiveEmbedding::Unknown { .. })) => {
                        ProfileCompatibility::UnknownActiveProfile
                    }
                    (Some(stored), Some(ActiveEmbedding::Configuration { profile_id, .. }))
                        if stored == profile_id =>
                    {
                        ProfileCompatibility::Compatible
                    }
                    _ => ProfileCompatibility::ProfileMismatch,
                };
                if std::mem::discriminant(&expected)
                    != std::mem::discriminant(&e.profile_compatibility)
                {
                    return None;
                }
                previous = e.id.get();
            }
            if let Some(token) = &d.next_cursor {
                let c = CatalogCursor::decode(token, r).ok()?;
                if c.namespace_after != Some(previous) {
                    return None;
                }
            }
            (d.limit, d.complete, &d.next_cursor)
        }
        (CatalogData::Tags(d), CatalogKind::Tags) => {
            let buckets = [&d.tags, &d.entities, &d.topics, &d.emotions];
            if d.complete != buckets.iter().all(|b| !b.truncated) {
                return None;
            }
            let next = d
                .next_cursor
                .as_deref()
                .map(|c| CatalogCursor::decode(c, r))
                .transpose()
                .ok()?;
            for (i, b) in buckets.into_iter().enumerate() {
                if b.items.len() > r.limit
                    || !valid_total(
                        b.total.get(),
                        b.items.len(),
                        incoming.as_ref().is_some_and(|c| c.tag_after[i].is_some()),
                        b.truncated,
                    )
                    || (b.truncated && b.items.len() != r.limit)
                {
                    return None;
                }
                if incoming
                    .as_ref()
                    .and_then(|c| c.tag_after[i].as_ref())
                    .is_none()
                    && !b.truncated
                    && b.total.get() != b.items.len() as u64
                {
                    return None;
                }
                let mut names = std::collections::BTreeSet::new();
                for (n, e) in b.items.iter().enumerate() {
                    if !names.insert(&e.name)
                        || incoming
                            .as_ref()
                            .and_then(|c| c.tag_after[i].as_ref())
                            .is_some_and(|a| *a == CatalogCursor::label_anchor(i, &e.name))
                    {
                        return None;
                    }
                    if e.name.is_empty()
                        || e.name.len() > crate::model::constants::LABEL_MAX_BYTES
                        || e.count.get() == 0
                        || e.count.get() > d.memories_counted.get()
                    {
                        return None;
                    }
                    let sum = e
                        .sources
                        .raw_tags
                        .get()
                        .checked_add(e.sources.canonical_legacy_retained_only.get())?
                        .checked_add(e.sources.canonical_native_v2.get())?;
                    if sum != e.count.get()
                        || (i == 0
                            && (e.sources.canonical_legacy_retained_only.get() != 0
                                || e.sources.canonical_native_v2.get() != 0))
                    {
                        return None;
                    }
                    if n > 0 {
                        let prev = &b.items[n - 1];
                        if prev.count < e.count || (prev.count == e.count && prev.name >= e.name) {
                            return None;
                        }
                    }
                }
                if let Some(next) = &next {
                    let expected = b
                        .items
                        .last()
                        .map(|e| CatalogCursor::label_anchor(i, &e.name))
                        .or_else(|| incoming.as_ref().and_then(|c| c.tag_after[i].clone()));
                    if next.tag_after[i] != expected {
                        return None;
                    }
                }
            }
            (d.limit, d.complete, &d.next_cursor)
        }
        _ => return None,
    };
    if limit != r.limit || complete != token.is_none() {
        return None;
    }
    if let Some(token) = token {
        let next = CatalogCursor::decode(token, r).ok()?;
        if next.at != p.at
            || next.namespace_id != selected
            || incoming
                .as_ref()
                .is_some_and(|c| c.catalog_sha256 != next.catalog_sha256)
        {
            return None;
        }
    }
    Some(())
}
