//! External benchmark framework for Recalld.
//!
//! Runs standardized benchmarks (LoCoMo, etc.) against external
//! datasets to produce comparable results for research papers.

pub mod claude;
pub mod locomo;

/// Synthetic stores and a deterministic embedding stub for the UI harness.
pub mod synthetic;

/// Private evaluator-channel consumer, never an actor MCP tool.
pub mod controls;

/// Shared-clock evaluator constructor and bounded isolated checkpoint controls.
pub mod frozen;

mod checkpoint;
mod native_prefix;
