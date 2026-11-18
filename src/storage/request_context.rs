//! Original public request identity for native compound workflows only.
//! Known historical native codecs omit this extension and retain contract2.
use super::StorageError;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RetainedRequestContext {
    proof_version: u32,
    contract_version: u32,
}
impl RetainedRequestContext {
    pub(crate) fn from_contract(contract: u32) -> Result<Option<Self>, StorageError> {
        match contract {
            1 => Ok(Some(Self {
                proof_version: 1,
                contract_version: 1,
            })),
            2 => Ok(None),
            _ => Err(StorageError::InvalidMutationContext(
                "unsupported request contract".into(),
            )),
        }
    }
    /// Call only after independently establishing a known native writer codec.
    pub(crate) fn native_contract(context: Option<&Self>) -> Result<u32, StorageError> {
        match context {
            None => Ok(2),
            Some(Self {
                proof_version: 1,
                contract_version: 1,
            }) => Ok(1),
            Some(_) => Err(StorageError::InvalidControl(
                "invalid retained request context".into(),
            )),
        }
    }
}
/// Missing field may use a historical default; explicit null never may.
pub(crate) fn deserialize_present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}
