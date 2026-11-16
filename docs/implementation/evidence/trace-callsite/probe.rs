//! Standalone dependency-mechanism probe, not attribution of the integration failure.
use std::io::Write;
use std::sync::{Arc, Mutex};
#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);
impl Write for Capture {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> { self.0.lock().unwrap().extend_from_slice(b); Ok(b.len()) }
    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Capture {
    type Writer=Self;
    fn make_writer(&'a self)->Self {self.clone()}
}
fn dispatch(c: Capture)->tracing::Dispatch {
    tracing::Dispatch::new(tracing_subscriber::fmt().json().without_time()
        .with_env_filter("off,recalld::performance=trace").with_writer(c).finish())
}
fn enabled_site()->bool {tracing::enabled!(target:"recalld::performance", tracing::Level::TRACE)}
fn existing_event() {tracing::trace!(target:"recalld::performance", kind="existing_site");}
fn fresh_event() {tracing::trace!(target:"recalld::performance", kind="fresh_site");}
fn count(c:&Capture)->usize {c.0.lock().unwrap().iter().filter(|b|**b==b'\n').count()}
fn main(){
    let capture=Capture::default();
    let active=dispatch(capture.clone());
    let before=std::thread::spawn(|| {let enabled=enabled_site();existing_event();enabled}).join().unwrap();
    assert!(!before);
    let selected=tracing::dispatcher::with_default(&active,|| {let selected=enabled_site();existing_event();selected});
    let after_existing=count(&capture);
    tracing::dispatcher::with_default(&active,fresh_event);
    let after_fresh=count(&capture);
    // Second dispatch is a diagnostic control only, not a recommended fix.
    let _second=dispatch(Capture::default());
    let after_registration=tracing::dispatcher::with_default(&active,|| {let selected=enabled_site();existing_event();selected});
    let after_second=count(&capture);
    println!("untraced_enabled={before} active_enabled={selected} rows_existing={after_existing} rows_after_fresh={after_fresh} active_enabled_after_second_dispatch={after_registration} rows_after_second={after_second}");
    println!("{}",String::from_utf8(capture.0.lock().unwrap().clone()).unwrap());
    assert!(!selected,"hypothesized cached-interest mechanism was not observed");
    assert_eq!(after_existing,0);
    assert_eq!(after_fresh,1,"active subscriber must receive a newly registered site");
    assert!(after_registration);
    assert_eq!(after_second,2);
}
