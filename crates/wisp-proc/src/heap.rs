//! glibc's malloc settings at the start (ADR-0087).

/// glibc's arenas `tune_heap` allows (ADR-0087): one per thread (up to 8 × cores, 32 on a 4-core box) scatters what a
/// server keeps over pages no trim can return; two is the usual cap for a threaded server.
pub const HEAP_ARENAS: i32 = 2;
/// Blocks this big or bigger get their own mapping, handed back on free (glibc's starting threshold, kept fixed).
pub const HEAP_MMAP_AT: i32 = 128 * 1024;

/// glibc's malloc settings read from the environment: when one is set, the user chose and `tune_heap` leaves glibc be.
const MALLOC_ENV: [&str; 3] = ["GLIBC_TUNABLES", "MALLOC_ARENA_MAX", "MALLOC_MMAP_THRESHOLD_"];

/// Is any of glibc's malloc settings in `vars` (the environment's names)?
fn malloc_env_set<S: AsRef<std::ffi::OsStr>>(vars: impl IntoIterator<Item = S>) -> bool {
    vars.into_iter().any(|v| MALLOC_ENV.iter().any(|m| v.as_ref() == std::ffi::OsStr::new(m)))
}

/// Caps glibc's arenas at `HEAP_ARENAS` and maps blocks of `HEAP_MMAP_AT` or more on their own (ADR-0087), unless the
/// environment sets glibc's malloc itself. Call first thing in `main`, before any thread starts. False when nothing
/// was changed (not glibc, or the environment chose).
pub fn tune_heap() -> bool {
    !malloc_env_set(std::env::vars_os().map(|(k, _)| k)) && crate::sys::tune_heap(HEAP_ARENAS, HEAP_MMAP_AT)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_malloc_setting_in_the_environment_leaves_glibc_to_it() {
        assert!(super::malloc_env_set(["PATH", "MALLOC_ARENA_MAX"]));
        assert!(super::malloc_env_set(["GLIBC_TUNABLES"]));
        assert!(super::malloc_env_set(["MALLOC_MMAP_THRESHOLD_"]));
        assert!(!super::malloc_env_set(["PATH", "HOME", "MALLOC_ARENA", "MALLOC_MMAP_THRESHOLD"]), "names match whole");
        assert!(!super::malloc_env_set(Vec::<String>::new()));
    }

    #[test]
    fn tuning_the_heap_is_glibcs_alone_and_safe_to_call_twice() {
        let glibc = cfg!(all(target_os = "linux", target_env = "gnu"));
        let chose = super::malloc_env_set(std::env::vars_os().map(|(k, _)| k));
        assert_eq!(super::tune_heap(), glibc && !chose);
        assert_eq!(super::tune_heap(), glibc && !chose, "mallopt takes the same settings again");
        let v: Vec<u8> = vec![7; usize::try_from(super::HEAP_MMAP_AT).unwrap_or(0) * 4];
        assert_eq!(v.iter().map(|b| u64::from(*b)).sum::<u64>(), 7 * 4 * 128 * 1024, "a mapped block still works");
    }
}
