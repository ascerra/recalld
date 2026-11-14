//! Live adapters and pinned query leases share exactly the same algorithms.
use std::{ops::Deref, sync::Arc};
use tokio::sync::{OwnedRwLockReadGuard, RwLock, RwLockReadGuard};

pub(super) enum ReadHandle<T> {
    Live(Arc<RwLock<T>>),
    Pinned(Arc<OwnedRwLockReadGuard<T>>),
}
pub(super) enum ReadGuard<'a, T> {
    Live(RwLockReadGuard<'a, T>),
    Pinned(&'a T),
}
impl<T> Deref for ReadGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        match self {
            Self::Live(value) => value,
            Self::Pinned(value) => value,
        }
    }
}
impl<T> ReadHandle<T> {
    pub fn read(&self) -> ReadGuard<'_, T> {
        match self {
            Self::Live(value) => ReadGuard::Live(tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(value.read())
            })),
            Self::Pinned(value) => ReadGuard::Pinned(value),
        }
    }
}

pub(super) enum FtsHandle {
    Live(Arc<tokio::sync::Mutex<super::FtsIndex>>),
    // Connection is Send, not Sync. Serialize access to the already owned lease;
    // never reacquire the live mutex while a publisher is queued behind it.
    Pinned(std::sync::Mutex<tokio::sync::OwnedMutexGuard<super::FtsIndex>>),
}
pub(super) enum FtsGuard<'a> {
    Live(tokio::sync::MutexGuard<'a, super::FtsIndex>),
    Pinned(std::sync::MutexGuard<'a, tokio::sync::OwnedMutexGuard<super::FtsIndex>>),
}
impl Deref for FtsGuard<'_> {
    type Target = super::FtsIndex;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Live(value) => value,
            Self::Pinned(value) => value,
        }
    }
}
impl FtsHandle {
    pub fn lock(&self) -> FtsGuard<'_> {
        match self {
            Self::Live(value) => FtsGuard::Live(tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(value.lock())
            })),
            Self::Pinned(value) => FtsGuard::Pinned(value.lock().expect("pinned FTS poisoned")),
        }
    }
}
