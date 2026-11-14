//! External benchmark framework for Recalld.
//!
//! Runs standardized benchmarks (LoCoMo, etc.) against external
//! datasets to produce comparable results for research papers.

pub mod claude;
pub mod locomo;

/// Private evaluator-channel consumer, never an actor MCP tool.
pub mod controls;

/// Shared-clock evaluator constructor; snapshot/reset remain unavailable.
pub mod frozen;

mod checkpoint;
