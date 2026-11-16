//! Serialization for Recalld memory records.
//!
//! Provides serde-based camelCase JSON format for the HTTP API wire protocol.
//! Uses `serde_json` with skip-if-none and integer-millis timestamps.

pub mod catalog;
pub mod contract;
mod json;
pub mod list;
pub mod native_batch;
pub mod native_single;
pub(crate) mod page_schema;
pub(crate) mod pair_cursor;
pub mod policy;
pub mod query;
mod query_explanation;
pub mod query_response;
pub(crate) mod query_schema;
pub mod scan;
pub mod session;
pub mod store_api1;

pub use json::{
    ApiError, ApiResponse, CreateMemoryRequest, MemoryResponse, NamespaceRequest,
    NamespaceResponse, SearchHit, SearchRequest, SearchResponse,
};

pub mod catalog_page;
pub(crate) mod native_lifecycle;

pub(crate) mod lifecycle_api1;

pub mod batch_api1;
