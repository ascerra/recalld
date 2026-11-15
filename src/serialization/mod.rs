//! Serialization for Recalld memory records.
//!
//! Provides serde-based camelCase JSON format for the HTTP API wire protocol.
//! Uses `serde_json` with skip-if-none and integer-millis timestamps.

pub mod contract;
mod json;
pub(crate) mod pair_cursor;
pub mod query;
mod query_explanation;
pub mod query_response;
pub mod scan;
pub mod session;

pub use json::{
    ApiError, ApiResponse, CreateMemoryRequest, MemoryResponse, NamespaceRequest,
    NamespaceResponse, SearchHit, SearchRequest, SearchResponse,
};
