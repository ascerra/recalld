use super::*;

fn filter_arguments(filter: &PipelineSearchFilter) -> Result<Value, InputError> {
    if !filter.exclude_tags.is_empty() {
        return Err(invalid(
            "INVALID_COMBINATION",
            "tags",
            "prepared RPC has no exclusion selector",
        ));
    }
    let mut value = json!({
        "contractVersion":2,
        "tags":filter.require_tags.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "entities":filter.labels.entities,"topics":filter.labels.topics,"emotions":filter.labels.emotions,
        "phases":filter.phases,"permastoreOnly":filter.permastore_only,
    });
    if let Some(strength) = filter.min_strength {
        if !strength.is_finite() {
            return Err(invalid(
                "OUT_OF_RANGE",
                "minStrength",
                "strength must be finite",
            ));
        }
        value["minStrength"] = json!(strength);
    }
    Ok(value)
}

fn version(mode: VersionMode) -> &'static str {
    match mode {
        VersionMode::Current => "current",
        VersionMode::History => "history",
    }
}

impl QueryRequest {
    pub(crate) fn rpc_arguments(&self) -> Result<Value, InputError> {
        let q = &self.query;
        if q.entities != q.filter.labels.entities {
            return Err(invalid(
                "INVALID_COMBINATION",
                "entities",
                "prepared query entity selectors differ",
            ));
        }
        if !q.min_score.is_finite()
            || q.query_embedding
                .as_ref()
                .is_some_and(|v| v.iter().any(|x| !x.is_finite()))
        {
            return Err(invalid(
                "OUT_OF_RANGE",
                "embedding",
                "prepared query numbers must be finite",
            ));
        }
        let mut value = filter_arguments(&q.filter)?;
        let object = value.as_object_mut().expect("constructed object");
        for (key, v) in [
            ("namespace", json!(q.namespace)),
            ("queryMode", json!(q.query_mode)),
            ("versionMode", json!(version(self.version_mode))),
            ("limit", json!(q.limit)),
            ("depth", json!(q.graph_depth)),
            ("minScore", json!(q.min_score)),
            ("includeGhosts", json!(q.include_ghosts)),
            ("recordAccess", json!(q.record_access)),
            ("compact", json!(!self.include_neighbors)),
            ("explain", json!(self.explain)),
            ("includeEmbeddings", json!(self.include_embeddings)),
        ] {
            object.insert(key.into(), v);
        }
        // Absence is significant; the shared parser intentionally rejects null
        // for these optional fields. Namespace is already session-resolved.
        for (key, value) in [
            ("query", q.text.as_ref().map(|v| json!(v))),
            ("ftsQuery", q.fts_query.as_ref().map(|v| json!(v))),
            ("embedding", q.query_embedding.as_ref().map(|v| json!(v))),
            (
                "embeddingProfileId",
                self.embedding_profile_id.as_ref().map(|v| json!(v)),
            ),
            ("timeRangeStart", q.time_range_start.map(|v| json!(v))),
            ("timeRangeEnd", q.time_range_end.map(|v| json!(v))),
        ] {
            if let Some(value) = value {
                object.insert(key.into(), value);
            }
        }
        Self::from_mcp(&value, &q.namespace)?;
        Ok(value)
    }
}

impl SimilarRequest {
    pub(crate) fn rpc_arguments(&self) -> Result<Value, InputError> {
        let crate::search::RetrievalSemantics::V2(mode) = self.options.semantics else {
            return Err(invalid(
                "INVALID_COMBINATION",
                "versionMode",
                "prepared similarity requires semantics2",
            ));
        };
        if self.options.original_contract_version != 2
            || self.options.min_score.is_some_and(|x| !x.is_finite())
        {
            return Err(invalid(
                "INVALID_COMBINATION",
                "contractVersion",
                "invalid prepared similarity selectors",
            ));
        }
        let mut value = filter_arguments(&self.options.filter)?;
        let object = value.as_object_mut().expect("constructed object");
        for (key, v) in [
            ("id", json!(self.id)),
            ("mode", json!("single")),
            ("versionMode", json!(version(mode))),
            (
                "namespaceScope",
                json!(match self.options.scope {
                    crate::search::SimilarityScope::SameNamespace => "sameNamespace",
                    crate::search::SimilarityScope::AllCompatible => "allCompatible",
                }),
            ),
            ("limit", json!(self.options.limit)),
            ("recordAccess", json!(self.options.record_access)),
            ("includeEmbeddings", json!(self.options.include_embeddings)),
            ("explain", json!(self.explain)),
        ] {
            object.insert(key.into(), v);
        }
        if let Some(ns) = &self.namespace {
            object.insert("namespace".into(), json!(ns));
        }
        if let Some(score) = self.options.min_score {
            object.insert("minScore".into(), json!(score));
        }
        Self::from_mcp(&value)?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_rpc_query_and_similar_preserve_all_selectors() {
        let args = json!({"contractVersion":2,"namespace":"named","query":"display text",
            "ftsQuery":"needle","embedding":[0.1,-0.3,1.0,0.0],
            "embeddingProfileId":format!("ep1:{}", "a".repeat(64)),"queryMode":"embedding_only",
            "versionMode":"history","tags":["test"],"entities":["José"],"topics":["two words"],
            "emotions":["calm"],"phases":["full","summary"],"minStrength":0.4,"minScore":0.3,
            "permastoreOnly":true,"includeGhosts":true,"limit":100,"depth":3,
            "timeRangeStart":-42,"timeRangeEnd":42,"recordAccess":false,"compact":false,
            "includeEmbeddings":true,"explain":true});
        let first = QueryRequest::from_mcp(&args, "wrong").unwrap();
        let wire = first.rpc_arguments().unwrap();
        let second = QueryRequest::from_mcp(&wire, "another-wrong").unwrap();
        assert_eq!(second.rpc_arguments().unwrap(), wire);
        assert_eq!(
            serde_json::to_value(&first.query).unwrap(),
            serde_json::to_value(&second.query).unwrap()
        );
        assert_eq!(
            first
                .query
                .query_embedding
                .unwrap()
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            second
                .query
                .query_embedding
                .unwrap()
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>()
        );
        let args = json!({"contractVersion":2,"id":"00000000-0000-0000-0000-000000000001",
            "namespace":"named","namespaceScope":"allCompatible","versionMode":"history",
            "entities":["Alice's"],"limit":100,"minScore":0.9,"recordAccess":false,
            "includeEmbeddings":true,"explain":true});
        let first = SimilarRequest::from_mcp(&args).unwrap();
        let wire = first.rpc_arguments().unwrap();
        assert_eq!(
            SimilarRequest::from_mcp(&wire)
                .unwrap()
                .rpc_arguments()
                .unwrap(),
            wire
        );
        assert_eq!(wire["namespaceScope"], "allCompatible");
        assert_eq!(wire["namespace"], "named");
    }

    #[test]
    fn prepared_rpc_rejects_internal_only_filters_instead_of_ignoring_them() {
        let mut request = QueryRequest::from_mcp(
            &json!({"contractVersion":2,
            "queryMode":"metadata_only","tags":["allow"]}),
            "default",
        )
        .unwrap();
        request
            .query
            .filter
            .exclude_tags
            .push(Tag::new("excluded").unwrap());
        assert!(request.rpc_arguments().is_err());
        request.query.filter.exclude_tags.clear();
        request.query.entities.push("silently different".into());
        assert!(request.rpc_arguments().is_err());
    }
}
