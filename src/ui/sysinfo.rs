//! Process resident memory and CPU time, for Health → System.

/// Current resident set size in bytes.
#[cfg(target_os = "macos")]
pub fn rss_bytes() -> Option<u64> {
    let mut info: libc::proc_taskinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_taskinfo>() as libc::c_int;
    // SAFETY: `info` is a properly sized, writable `proc_taskinfo`.
    let n = unsafe {
        libc::proc_pidinfo(
            std::process::id() as libc::c_int,
            libc::PROC_PIDTASKINFO,
            0,
            (&mut info as *mut libc::proc_taskinfo).cast(),
            size,
        )
    };
    (n == size).then_some(info.pti_resident_size)
}

/// Current resident set size in bytes.
#[cfg(target_os = "linux")]
pub fn rss_bytes() -> Option<u64> {
    let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
    let pages: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
    // SAFETY: sysconf has no preconditions.
    let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    (page > 0).then(|| pages * page as u64)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn rss_bytes() -> Option<u64> {
    None
}

/// User + system CPU time consumed by this process, in seconds.
pub fn cpu_seconds() -> Option<f64> {
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    // SAFETY: `usage` is a writable `rusage`.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) } != 0 {
        return None;
    }
    let secs = |t: libc::timeval| t.tv_sec as f64 + t.tv_usec as f64 / 1e6;
    Some(secs(usage.ru_utime) + secs(usage.ru_stime))
}

#[cfg(test)]
mod tests {
    #[test]
    fn reports_plausible_values() {
        let rss = super::rss_bytes().expect("rss on supported platforms");
        assert!(rss > 1 << 20, "rss {rss}");
        let a = super::cpu_seconds().unwrap();
        let mut x = 0u64;
        for i in 0..5_000_000u64 {
            x = x.wrapping_add(i * i);
        }
        std::hint::black_box(x);
        assert!(super::cpu_seconds().unwrap() >= a);
    }
}
