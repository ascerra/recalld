//! Internal opt-in performance events. No payloads, memory identities or readiness claims.
//! Enable only the `recalld::performance=trace` target. Disabled timers do not
//! sample clocks or allocate. Sink overhead remains in caller-observed latency.
//! Ordinary writer I/O errors cannot replace domain outcomes. Subscriber panics
//! are not caught: no unwind-safety guarantee is made for arbitrary subscribers.
use std::future::Future;
use std::num::NonZeroU64;
use std::ops::{Deref, DerefMut};
use std::pin::{Pin, pin};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
static NEXT_TRACE: AtomicU64 = AtomicU64::new(1);
use tracing::Instrument;

// A root decision lives on the polling task, including cancellation drops. The
// same Copy value is carried into blocking workers with sync_scope; no Arc/box.
tokio::task_local! { static REQUEST: Decision; }
#[derive(Clone, Copy)]
struct Decision {
    selected: bool,
    every: u64,
    root_ordinal: u64,
    sampled_roots_seen: u64,
}
struct Sampler {
    every: Option<NonZeroU64>,
    roots: AtomicU64,
    selected: AtomicU64,
}
impl Sampler {
    fn new(every: Option<NonZeroU64>) -> Self {
        Self {
            every,
            roots: AtomicU64::new(0),
            selected: AtomicU64::new(0),
        }
    }
    fn decide(&self) -> Decision {
        let root_ordinal = self.roots.fetch_add(1, Ordering::Relaxed) + 1;
        let every = self.every.map_or(0, NonZeroU64::get);
        let selected = every != 0 && selected(root_ordinal, every);
        let sampled_roots_seen = if selected {
            self.selected.fetch_add(1, Ordering::Relaxed) + 1
        } else {
            0
        };
        Decision {
            selected,
            every,
            root_ordinal,
            sampled_roots_seen,
        }
    }
}
// Fixed SplitMix64 permutation avoids cadence aliasing with periodic workloads.
// This is deterministic sequence sampling, not random traffic/quality evidence.
fn selected(ordinal: u64, every: u64) -> bool {
    let mut z = ordinal.wrapping_add(0x9e3779b97f4a7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    ((z ^ (z >> 31)) % every) == 0
}
fn parse_rate(value: Option<&str>) -> Option<NonZeroU64> {
    value.unwrap_or("1").parse::<NonZeroU64>().ok()
}
fn sampler() -> &'static Sampler {
    static SAMPLER: OnceLock<Sampler> = OnceLock::new();
    SAMPLER.get_or_init(|| {
        // Read once, only when tracing is enabled. Invalid values disable this
        // instrumentation, never change a domain result or silently select full.
        let rate = match std::env::var("RECALLD_PERFORMANCE_SAMPLE_EVERY") {
            Ok(value) => parse_rate(Some(&value)),
            Err(std::env::VarError::NotPresent) => parse_rate(None),
            Err(_) => None,
        };
        Sampler::new(rate)
    })
}
fn target_enabled() -> bool {
    // Contain tracing-core#3611: an untraced thread must not first register
    // this callsite against NoSubscriber while a scoped Dispatch is alive.
    // Keep the global/static fast path before inspecting thread-local state.
    if tracing::Level::TRACE > tracing::level_filters::STATIC_MAX_LEVEL
        || tracing::Level::TRACE > tracing::level_filters::LevelFilter::current()
        || tracing::dispatcher::get_default(|d| d.is::<tracing::subscriber::NoSubscriber>())
    {
        return false;
    }
    // Outside get_default: nesting would trigger dispatcher reentrancy suppression.
    tracing::enabled!(target: "recalld::performance", tracing::Level::TRACE)
}
pub(crate) fn enabled() -> bool {
    REQUEST
        .try_with(|d| d.selected)
        .unwrap_or_else(|_| target_enabled())
}
// Gate before evaluating fields, not only inside the tracing subscriber. All
// performance events, including counters outside Timer, must use this gate.
macro_rules! event {
    ($condition:expr; $($fields:tt)*) => {
        if $condition {
            $crate::performance::event!($($fields)*);
        }
    };
    ($($fields:tt)*) => {
        if $crate::performance::enabled() {
            tracing::trace!(target: "recalld::performance", $($fields)*);
        }
    };
}
pub(crate) use event;

pub(crate) struct Timer {
    stage: &'static str,
    start: Option<Instant>,
}
impl Timer {
    pub fn new(stage: &'static str) -> Self {
        Self::new_if(stage, true)
    }
    pub fn new_if(stage: &'static str, capture: bool) -> Self {
        Self {
            stage,
            start: (capture && enabled()).then(Instant::now),
        }
    }
    fn end(&mut self, outcome: &'static str) {
        if let Some(start) = self.start.take() {
            crate::performance::event!(
                kind = "duration",
                stage = self.stage,
                outcome,
                elapsed_ns = start.elapsed().as_nanos() as u64
            );
        }
    }
    pub fn outcome(mut self, success: bool) {
        self.end(if success { "ok" } else { "error" });
    }
    pub fn complete(mut self) {
        self.end("ok");
    }
}
impl Drop for Timer {
    fn drop(&mut self) {
        self.end("cancelled_error_or_unwound");
    }
}

/// Consecutive marks partition pipeline wall time, including a final residual.
/// Nested adapter/lock events must not be added again to these intervals.
pub(crate) struct Timeline {
    start: Option<Instant>,
    last: Option<Instant>,
    accounted: u64,
}
impl Timeline {
    pub fn new() -> Self {
        Self::new_if(true)
    }
    pub fn new_if(capture: bool) -> Self {
        let start = (capture && enabled()).then(Instant::now);
        Self {
            start,
            last: start,
            accounted: 0,
        }
    }
    pub fn mark(&mut self, stage: &'static str) {
        if let Some(last) = self.last {
            let now = Instant::now();
            let elapsed = now.duration_since(last).as_nanos() as u64;
            self.accounted += elapsed;
            self.last = Some(now);
            crate::performance::event!(kind = "exclusive_pipeline", stage, elapsed_ns = elapsed);
        }
    }
}
impl Drop for Timeline {
    fn drop(&mut self) {
        self.mark("residual");
        if let (Some(start), Some(end)) = (self.start, self.last) {
            crate::performance::event!(
                kind = "pipeline_accounting",
                elapsed_ns = end.duration_since(start).as_nanos() as u64,
                accounted_ns = self.accounted
            );
        }
    }
}

/// An outer request reports error explicitly; cancellation/unwind retains a
/// duration with an honestly ambiguous outcome. Errors themselves are not logged.
pub(crate) fn measure<T, E>(
    operation: &'static str,
    record_access: bool,
    future: impl Future<Output = Result<T, E>>,
) -> impl Future<Output = Result<T, E>> {
    measure_with_sampler(operation, record_access, future, None)
}

fn measure_with_sampler<T, E>(
    operation: &'static str,
    record_access: bool,
    future: impl Future<Output = Result<T, E>>,
    sampler_override: Option<&Sampler>,
) -> impl Future<Output = Result<T, E>> {
    measure_request(operation, Some(record_access), future, sampler_override)
}

/// Transport intent is unknown before decoding, never implicitly read-only.
pub(crate) fn measure_transport<T, E>(
    operation: &'static str,
    future: impl Future<Output = Result<T, E>>,
) -> impl Future<Output = Result<T, E>> {
    measure_request(operation, None, future, None)
}

async fn measure_request<T, E>(
    operation: &'static str,
    record_access: Option<bool>,
    future: impl Future<Output = Result<T, E>>,
    sampler_override: Option<&Sampler>,
) -> Result<T, E> {
    let decision = REQUEST.try_with(|d| *d).unwrap_or_else(|_| {
        if target_enabled() {
            sampler_override.unwrap_or_else(|| sampler()).decide()
        } else {
            Decision {
                selected: false,
                every: 0,
                root_ordinal: 0,
                sampled_roots_seen: 0,
            }
        }
    });
    // Keep a single future-containing path: duplicating it in sampled and
    // unsampled branches compounds stack usage when request wrappers nest.
    // Full trace keeps its original event-v1 span fields. Sampled traces have
    // version 2 and explicit root-level counts (not whole-population metrics).
    let span = if !decision.selected {
        tracing::Span::none()
    } else {
        let trace_id = NEXT_TRACE.fetch_add(1, Ordering::Relaxed);
        if record_access.is_none() {
            tracing::trace_span!(target: "recalld::performance", "performance_request",
            operation, intent = "unknown", trace_id, event_version = 3,
            sample_every = decision.every, root_ordinal = decision.root_ordinal,
            sampled_roots_seen = decision.sampled_roots_seen)
        } else if decision.every == 1 {
            tracing::trace_span!(target: "recalld::performance", "performance_request",
            operation, record_access = record_access.unwrap(), trace_id, event_version = 1)
        } else {
            tracing::trace_span!(target: "recalld::performance", "performance_request",
            operation, record_access = record_access.unwrap(), trace_id, event_version = 2,
            sample_every = decision.every, root_ordinal = decision.root_ordinal,
            sampled_roots_seen = decision.sampled_roots_seen)
        }
    };
    let mut inner = pin!(Some(async move {
        let mut timer = Timer::new(operation);
        let result = future.await;
        timer.end(if result.is_ok() { "ok" } else { "error" });
        result
    }));
    let mut drop_context = RequestDrop {
        inner: inner.as_mut(),
        decision,
        span: span.clone(),
    };
    REQUEST
        .scope(
            decision,
            std::future::poll_fn(|cx| drop_context.inner.as_mut().as_pin_mut().unwrap().poll(cx))
                .instrument(span),
        )
        .await
}

// WithSubscriber restores its dispatcher for polling, but not for task abort.
// Drop the stack-pinned future under the original decision/subscriber/span so
// its timers and owned work guards retain context even when another task aborts
// it. Pin::set drops in place: no unsafe projection, box or added dependency.
struct RequestDrop<'a, F> {
    inner: Pin<&'a mut Option<F>>,
    decision: Decision,
    span: tracing::Span,
}
impl<F> Drop for RequestDrop<'_, F> {
    fn drop(&mut self) {
        let Self {
            inner,
            decision,
            span,
        } = self;
        let mut drop_inner = || REQUEST.sync_scope(*decision, || inner.set(None));
        if span
            .with_subscriber(|(_, dispatch)| {
                tracing::dispatcher::with_default(dispatch, || span.in_scope(&mut drop_inner));
            })
            .is_none()
        {
            drop_inner();
        }
    }
}

/// Canonical receipt identities are isolated trace fields, never metric labels.
/// Do not fabricate receipts, canonical read generations or readiness barriers.
pub(crate) fn receipts(receipts: &[crate::storage::control::WriteReceipt]) {
    if enabled() {
        for receipt in receipts {
            crate::performance::event!( kind = "committed_receipt",
                store_id = %receipt.store_id, operation_id = %receipt.operation_id,
                committed_generation = receipt.committed_generation.get());
        }
    }
}

/// Counts this explicit Rust sync call, not opaque backend syscalls.
pub(crate) fn sync_io<T>(
    resource: &'static str,
    method: &'static str,
    sync: impl FnOnce() -> std::io::Result<T>,
) -> std::io::Result<T> {
    if !enabled() {
        return sync();
    }
    event!(kind = "sync_call", resource, method, phase = "attempt");
    let timer = Timer::new("storage.sync");
    let result = sync();
    event!(
        kind = "sync_call",
        resource,
        method,
        phase = "return",
        success = result.is_ok()
    );
    timer.outcome(result.is_ok());
    result
}

/// Carry the request's subscriber and span into a blocking worker. No capture
/// occurs when disabled. Queue delay ends when the closure actually begins.
pub(crate) struct Worker {
    context: Option<(tracing::Dispatch, tracing::Span)>,
    queued: Timer,
    decision: Option<Decision>,
}
impl Worker {
    pub fn new(stage: &'static str) -> Self {
        Self::new_if(stage, true)
    }
    // Keep the inherited subscriber/sampling decision for existing child work,
    // even when this caller does not claim queue-duration coverage.
    pub fn new_if(stage: &'static str, capture_queue: bool) -> Self {
        let context = enabled().then(|| {
            (
                tracing::dispatcher::get_default(Clone::clone),
                tracing::Span::current(),
            )
        });
        // One decision: a disabled context never starts a clock or emits on drop.
        let queued = Timer {
            stage,
            start: context
                .as_ref()
                .filter(|_| capture_queue)
                .map(|_| Instant::now()),
        };
        Self {
            context,
            queued,
            decision: REQUEST.try_with(|d| *d).ok(),
        }
    }
    pub fn run<T>(self, work: impl FnOnce() -> T) -> T {
        let Self {
            context,
            queued,
            decision,
        } = self;
        let run = || match context {
            Some((dispatch, span)) => tracing::dispatcher::with_default(&dispatch, || {
                span.in_scope(|| {
                    queued.complete();
                    work()
                })
            }),
            None => work(),
        };
        match decision {
            Some(decision) => REQUEST.sync_scope(decision, run),
            None => run(),
        }
    }
}

/// Release the actual guard before reporting its hold duration; never unlock
/// a borrowed FD or change the acquisition/poisoning contract.
pub(crate) struct Hold<G> {
    guard: Option<G>,
    timer: Timer,
}
impl<G> Hold<G> {
    pub fn new(stage: &'static str, guard: G) -> Self {
        Self::new_if(stage, guard, true)
    }
    pub fn new_if(stage: &'static str, guard: G, capture: bool) -> Self {
        Self {
            guard: Some(guard),
            timer: Timer::new_if(stage, capture),
        }
    }
}
impl<G: Deref> Deref for Hold<G> {
    type Target = G::Target;
    fn deref(&self) -> &Self::Target {
        self.guard.as_ref().unwrap().deref()
    }
}
impl<G: DerefMut> DerefMut for Hold<G> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard.as_mut().unwrap().deref_mut()
    }
}
impl<G> Drop for Hold<G> {
    fn drop(&mut self) {
        drop(self.guard.take());
        self.timer.end("released");
    }
}

#[cfg(test)]
#[path = "performance_tests.rs"]
pub(crate) mod tests;
