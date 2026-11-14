//! Opaque exact-pair continuation values. These are not authorization tokens or
//! proof of a ReadView. Actual capture and post-comparison validation are required.
use super::{
    contract::InputError,
    scan::{ExactScanRequest, MAX_CURSOR_BYTES},
};
use crate::{model::NamespaceId, storage::control::GenerationRef};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};

pub(crate) const PAIR_ELIGIBILITY_VERSION: u32 = 1;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PairCursor {
    pub cursor_version: u32,
    pub contract_version: u32,
    pub at: GenerationRef,
    pub namespace_id: NamespaceId,
    pub namespace_sha256: [u8; 32],
    pub profile_id: String,
    pub threshold_bits: u64,
    pub eligibility_version: u32,
    pub next_pair: u64,
}
fn invalid() -> InputError {
    InputError {
        code: "INVALID_CURSOR",
        field: "/cursor".into(),
        message: "cursor is malformed, unsupported, or differently bound",
    }
}
impl PairCursor {
    pub(crate) fn encode(&self) -> Result<String, InputError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| invalid())?;
        let encoded = URL_SAFE_NO_PAD.encode(bytes);
        if encoded.len() > MAX_CURSOR_BYTES {
            return Err(invalid());
        }
        Ok(encoded)
    }
    pub(crate) fn decode(value: &str, request: &ExactScanRequest) -> Result<Self, InputError> {
        if value.is_empty() || value.len() > MAX_CURSOR_BYTES {
            return Err(invalid());
        }
        let bytes = URL_SAFE_NO_PAD.decode(value).map_err(|_| invalid())?;
        if URL_SAFE_NO_PAD.encode(&bytes) != value {
            return Err(invalid());
        }
        let cursor: Self = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
        cursor.validate()?;
        if cursor.threshold_bits != request.threshold.to_bits() {
            return Err(invalid());
        }
        Ok(cursor)
    }
    fn validate(&self) -> Result<(), InputError> {
        let threshold = f64::from_bits(self.threshold_bits);
        let profile = self.profile_id.strip_prefix("ep1:").ok_or_else(invalid)?;
        if self.cursor_version != 1
            || self.contract_version != 2
            || self.eligibility_version != PAIR_ELIGIBILITY_VERSION
            || self.at.store_id.is_nil()
            || self.namespace_id.is_unset()
            || !threshold.is_finite()
            || !(0.0..=1.0).contains(&threshold)
            || (threshold == 0.0 && self.threshold_bits != 0)
            || profile.len() != 64
            || !profile
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(invalid());
        }
        Ok(())
    }
    /// Called only after actual snapshot scope/profile capture. Position is
    /// validated against the eligible record count by the comparator.
    pub(crate) fn matches_profile(&self, profile: &str) -> Result<(), InputError> {
        if self.profile_id != profile {
            return Err(invalid());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::control::Generation;
    use serde_json::json;
    use uuid::Uuid;
    #[test]
    fn exact_pair_cursor_strict_codec_roundtrip_and_bindings() {
        // Authored codec fixture; not a real snapshot or generation proof.
        let request = ExactScanRequest::from_http(&json!({}), "default").unwrap();
        let cursor = PairCursor {
            cursor_version: 1,
            contract_version: 2,
            at: GenerationRef {
                store_id: Uuid::from_u128(1),
                generation: Generation::MAX,
            },
            namespace_id: NamespaceId::new(1),
            namespace_sha256: [7; 32],
            profile_id: format!("ep1:{}", "a".repeat(64)),
            threshold_bits: request.threshold.to_bits(),
            eligibility_version: 1,
            next_pair: 10,
        };
        let token = cursor.encode().unwrap();
        assert_eq!(PairCursor::decode(&token, &request).unwrap(), cursor);
        let wire: serde_json::Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(&token).unwrap()).unwrap();
        assert_eq!(wire["at"]["generation"], "9223372036854775807");
        let mut different = request.clone();
        different.threshold = 0.9;
        assert!(PairCursor::decode(&token, &different).is_err());
        for (field, value) in [
            ("cursorVersion", json!(2)),
            ("contractVersion", json!(1)),
            ("eligibilityVersion", json!(2)),
            ("namespaceId", json!(0)),
            ("profileId", json!("same-dimension")),
            ("thresholdBits", json!(f64::NAN.to_bits())),
            ("unknown", json!(true)),
        ] {
            let mut modified = wire.clone();
            modified[field] = value;
            let token = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&modified).unwrap());
            assert!(PairCursor::decode(&token, &request).is_err(), "{field}");
        }
        for value in [
            "".to_owned(),
            format!("{token}="),
            "!".into(),
            "x".repeat(MAX_CURSOR_BYTES + 1),
        ] {
            assert!(PairCursor::decode(&value, &request).is_err());
        }
        assert!(
            cursor
                .matches_profile(&format!("ep1:{}", "b".repeat(64)))
                .is_err()
        );
    }
}
