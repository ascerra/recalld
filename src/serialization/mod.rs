//! Serialization for Recalld memory records.
//!
//! Provides serde-based camelCase JSON format for the HTTP API wire protocol.
//! Uses `serde_json` with skip-if-none and integer-millis timestamps.

pub mod contract;
mod json;
pub mod native_batch;
pub mod native_single;
pub(crate) mod pair_cursor;
pub mod scan;
pub mod session;

pub use json::{
    ApiError, ApiResponse, CreateMemoryRequest, MemoryResponse, NamespaceRequest,
    NamespaceResponse, SearchHit, SearchRequest, SearchResponse,
};

pub(crate) mod native_lifecycle;
