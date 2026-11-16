# Fixture expectation provenance

Exact parent: `508d98c65b0f824bbcc0290812c63e0dea426edd` (PR148).

Run005 incorrectly expected zero-vector validation 400; inherited native-single
projection is semantic422 for `/embedding`. Run006 incorrectly generalized422
to initialStability; existing mapper returns400 for that field. This is inherited
prepared-native taxonomy, not proof the old public native route accepted stores.
Run006 expected lowercased entity display; the unchanged legacy route uses
MemoryResponse::from_cached and parse_structured_tags, which capitalizes display
names. Canonical tags remain lowercase. No production change was made to satisfy
either corrected expectation.

## src/serialization/native_single.rs:303

```rust
        let scope = self
            .namespace_id
            .zip(self.namespace)
            .map(|(namespace_id, namespace)| Scope::Namespace {
                namespace_id,
                namespace,
            });
        let (status, mut envelope) = if let Some(error) = self.error {
            let status = match error.category {
                ErrorCategory::InvalidInput
                    if error.code == "OUT_OF_RANGE"
                        && error.field.as_deref() == Some("/embedding") =>
                {
                    422
                }
                ErrorCategory::InvalidInput => 400,
                ErrorCategory::NotFound => 404,
                ErrorCategory::Conflict => 409,
                ErrorCategory::Unavailable => 503,
                _ => 500,
            };
            (status, Envelope::failure(scope, error))
```

## src/api/handlers.rs:298

```rust
        // The prefixes come from `constants` because the label length limits
        // reserve room for exactly these bytes.
        let mut merged_tags = req.tags.clone();
        for entity in &req.entities {
            let tag = format!("{ENTITY_TAG_PREFIX}{}", entity.to_lowercase());
            if !merged_tags.contains(&tag) {
                merged_tags.push(tag);
            }
```

## src/api/handlers.rs:425

```rust
        let mut response = MemoryResponse::from_cached(&memory, ns.name.clone());
        response.full_text = req.full_text;
        response.supersedes = supersedes_outcome;
        response.near_duplicates = near_duplicates;
```

## src/serialization/json.rs:153

```rust
impl MemoryResponse {
    /// Constructs a `MemoryResponse` from a `CachedRecord` and
    /// resolved namespace name. Embedding and access history
    /// are set to `None` -- the caller populates them if requested.
    pub fn from_cached(record: &CachedRecord, namespace_name: String) -> Self {
        let metadata = structured_metadata(&record.tags, record.labels.as_ref());
        Self {
            labels: record.labels.clone(),
            id: record.id,
            namespace: namespace_name,
            created_at: record.created_at,
            last_accessed_at: record.last_accessed_at,
            summary: record.summary.clone(),
            full_text: None, // loaded on demand from fulltext.dat
            tags: record.tags.clone(),
            entities: metadata.entities,
            topics: metadata.topics,
            emotions: metadata.emotions,
            phase: record.phase,
```

## src/model/tag.rs:306

```rust
pub fn parse_structured_tags(tags: &[Tag]) -> StructuredMetadata {
    let mut result = StructuredMetadata::default();
    for tag in tags {
        let s = tag.as_str();
        if let Some(name) = s.strip_prefix("entity/") {
            if !name.is_empty() {
                let display_name = name
                    .split_whitespace()
                    .map(|w| {
                        let mut chars = w.chars();
                        match chars.next() {
                            Some(c) => c.to_uppercase().to_string() + chars.as_str(),
                            None => String::new(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                result.entities.push(display_name);
            }
        } else if let Some(topic) = s.strip_prefix("topic/") {
            if !topic.is_empty() {
                result.topics.push(topic.to_string());
            }
        } else if let Some(emotion) = s.strip_prefix("emotion/") {
```
