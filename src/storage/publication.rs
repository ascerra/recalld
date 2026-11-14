//! Owned derived publication stamps. Canonical generation is always read from
//! the real metadata transaction, never inferred from this registry or an epoch.
use std::sync::Mutex;

use super::control::GenerationRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DerivedView {
    Graph,
    Vectors,
    Fts,
    Entities,
}

pub(crate) const REQUIRED_VIEWS: [DerivedView; 4] = [
    DerivedView::Graph,
    DerivedView::Vectors,
    DerivedView::Fts,
    DerivedView::Entities,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PublicationState {
    pub canonical: GenerationRef,
    pub applied: [Option<GenerationRef>; 4],
}

impl PublicationState {
    pub fn matches_canonical(&self) -> bool {
        self.applied.iter().all(|at| *at == Some(self.canonical))
    }
}

#[derive(Debug, Default)]
pub(crate) struct PublicationRegistry {
    applied: Mutex<[Option<GenerationRef>; 4]>,
    pub changed: tokio::sync::Notify,
}

impl PublicationRegistry {
    /// `actual` must come from the caller's pinned canonical read transaction.
    /// This comparison also rejects a commit whose invalidation hook was missed.
    pub(super) fn state(&self, actual: GenerationRef) -> PublicationState {
        PublicationState {
            canonical: actual,
            applied: *self.applied.lock().unwrap_or_else(|e| e.into_inner()),
        }
    }

    /// Never move an untouched view forward merely because metadata committed.
    pub(super) fn canonical_committed(&self) {
        self.changed.notify_waiters();
    }

    /// An in-place publisher clears its old stamp before modifying its view.
    pub(super) fn invalidate(&self, view: DerivedView) {
        self.applied.lock().unwrap_or_else(|e| e.into_inner())[view as usize] = None;
        self.changed.notify_waiters();
    }

    /// The metadata owner checks `at` against its actual current read transaction.
    /// The caller must have completed this particular view under its write lease.
    pub(super) fn published(&self, view: DerivedView, at: GenerationRef) {
        self.applied.lock().unwrap_or_else(|e| e.into_inner())[view as usize] = Some(at);
        self.changed.notify_waiters();
    }
}
