//! Where the server's resident memory is, part by part (reported, not gated): Linux reads `/proc/<pid>/smaps`, so a
//! CI run says whether the idle RSS is glibc's heaps, thread stacks, the binary's pages or something else.

/// glibc's `HEAP_MAX_SIZE` on 64-bit: each non-main arena's heap is mapped at an address aligned to it.
const ARENA_ALIGN: u64 = 64 * 1024 * 1024;
/// pthread's default guard: one page below each thread's stack.
const GUARD: u64 = 4096;

/// The resident set by part, in KB.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Parts {
    /// file-backed pages of the server's own binary (text, rodata)
    pub binary: u64,
    /// file-backed pages of anything else (libc, the database's `-shm`, …)
    pub files: u64,
    /// `[heap]`: glibc's main arena (brk)
    pub heap: u64,
    /// glibc's other arenas (one per thread, up to 8 × cores): count and resident KB
    pub arenas: (usize, u64),
    /// threads' stacks (an anonymous mapping just above a one-page guard), `[stack]` included
    pub stacks: (usize, u64),
    /// the rest of the anonymous memory (glibc's mmap'd big chunks, …)
    pub anon: u64,
    /// the three biggest of those, KB
    pub anon_top: Vec<u64>,
    /// the process's threads
    pub threads: Option<usize>,
}

struct Vma<'a> {
    start: u64,
    end: u64,
    perms: &'a str,
    path: &'a str,
    rss: u64,
}

fn vmas(smaps: &str) -> Vec<Vma<'_>> {
    let mut out: Vec<Vma<'_>> = Vec::new();
    for line in smaps.lines() {
        let mut f = line.split_whitespace();
        let Some(first) = f.next() else { continue };
        if let Some((a, b)) = first.split_once('-')
            && let (Ok(start), Ok(end)) = (u64::from_str_radix(a, 16), u64::from_str_radix(b, 16))
        {
            let perms = f.next().unwrap_or("");
            // offset, dev, inode, then the path (which may hold spaces)
            let path = line.splitn(6, char::is_whitespace).nth(5).unwrap_or("").trim();
            out.push(Vma { start, end, perms, path, rss: 0 });
        } else if first == "Rss:"
            && let Some(v) = out.last_mut()
        {
            v.rss = f.next().and_then(|n| n.parse().ok()).unwrap_or(0);
        }
    }
    out
}

/// Sorts `smaps` (the text of `/proc/<pid>/smaps`) into parts; `exe` is the binary's path as the kernel shows it.
pub fn parts_of(smaps: &str, exe: &str) -> Parts {
    let v = vmas(smaps);
    let mut p = Parts::default();
    let mut top = Vec::new();
    for (i, m) in v.iter().enumerate() {
        if m.path == exe {
            p.binary += m.rss;
        } else if m.path == "[heap]" {
            p.heap += m.rss;
        } else if m.path == "[stack]" {
            p.stacks.0 += 1;
            p.stacks.1 += m.rss;
        } else if m.path.starts_with('/') {
            p.files += m.rss;
        } else if (m.path.is_empty() || m.path.starts_with("[anon")) && m.perms.starts_with("rw") {
            // newer glibc may name its mappings (`[anon: glibc: malloc arena]`, `… pthread user stack: <tid>]`)
            let named = |what: &str| m.path.contains(what);
            let guarded = i > 0 && v[i - 1].end == m.start && v[i - 1].perms.starts_with("---") && v[i - 1].end - v[i - 1].start == GUARD;
            if m.start % ARENA_ALIGN == 0 || named("malloc arena") {
                p.arenas.0 += 1;
                p.arenas.1 += m.rss;
            } else if guarded || named("stack") {
                p.stacks.0 += 1;
                p.stacks.1 += m.rss;
            } else {
                p.anon += m.rss;
                top.push(m.rss);
            }
        }
        // the rest ([vdso], [vvar], PROT_NONE reserves) has no resident pages worth naming
    }
    top.sort_unstable_by(|a, b| b.cmp(a));
    top.truncate(3);
    p.anon_top = top;
    p
}

/// The parts of `pid`'s resident set (Linux), else only its thread count (macOS: `ps -M`).
pub fn parts(pid: u32) -> Option<Parts> {
    if cfg!(target_os = "linux") {
        let smaps = std::fs::read_to_string(format!("/proc/{pid}/smaps")).ok()?;
        let exe = std::fs::read_link(format!("/proc/{pid}/exe")).ok()?;
        let mut p = parts_of(&smaps, &exe.to_string_lossy());
        let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
        p.threads = status.lines().find_map(|l| l.strip_prefix("Threads:")).and_then(|n| n.trim().parse().ok());
        Some(p)
    } else {
        let o = std::process::Command::new("ps").args(["-M", "-p", &pid.to_string()]).output().ok()?;
        let lines = String::from_utf8_lossy(&o.stdout).lines().count();
        Some(Parts { threads: Some(lines.saturating_sub(1)), ..Parts::default() })
    }
}

#[allow(clippy::cast_precision_loss)]
fn mb(kb: u64) -> f64 {
    kb as f64 / 1024.0
}

impl Parts {
    /// One line: "binary 24.1 · other files 2.0 · [heap] 30.0 · 9 arenas 41.2 · 23 stacks 0.9 · other anon 12.0 MB
    /// (biggest 8.0, 2.0, 1.0); 23 threads". Without smaps (macOS), the threads only.
    pub fn line(&self) -> String {
        let threads = self.threads.map_or_else(String::new, |t| format!("{t} threads"));
        if *self == (Self { threads: self.threads, ..Self::default() }) {
            return threads;
        }
        let top: Vec<String> = self.anon_top.iter().map(|k| format!("{:.1}", mb(*k))).collect();
        format!(
            "binary {:.1} · other files {:.1} · [heap] {:.1} · {} arenas {:.1} · {} stacks {:.1} · other anon {:.1} MB (biggest {}){}",
            mb(self.binary),
            mb(self.files),
            mb(self.heap),
            self.arenas.0,
            mb(self.arenas.1),
            self.stacks.0,
            mb(self.stacks.1),
            mb(self.anon),
            if top.is_empty() { "none".to_owned() } else { top.join(", ") },
            if threads.is_empty() { String::new() } else { format!("; {threads}") }
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const SMAPS: &str = "\
55d0c0000000-55d0c1000000 r-xp 00000000 08:01 123 /home/runner/work/wisp/target/budgets/wisp
Size:              16384 kB
Rss:                8000 kB
55d0c2000000-55d0c2400000 rw-p 00000000 00:00 0 [heap]
Rss:                3000 kB
7f0000000000-7f0000800000 rw-p 00000000 00:00 0
Rss:                2048 kB
7f0000800000-7f0004000000 ---p 00000000 00:00 0
Rss:                   0 kB
7f1000000000-7f1000001000 ---p 00000000 00:00 0
Rss:                   0 kB
7f1000001000-7f1000201000 rw-p 00000000 00:00 0
Rss:                  16 kB
7f1000300000-7f1000500000 rw-p 00000000 00:00 0
Rss:                 900 kB
7f1000600000-7f1000610000 rw-p 00000000 00:00 0
Rss:                  64 kB
7f2000000000-7f2000100000 r-xp 00000000 08:01 9 /usr/lib/x86_64-linux-gnu/libc.so.6
Rss:                 700 kB
7f2000200000-7f2000208000 rw-s 00000000 08:01 10 /tmp/home with space/wisp.db-shm
Rss:                  32 kB
7ffd00000000-7ffd00021000 rw-p 00000000 00:00 0 [stack]
Rss:                  40 kB
7ffd00100000-7ffd00102000 r-xp 00000000 00:00 0 [vdso]
Rss:                   8 kB
";

    /// glibc ≥ 2.40 with `glibc.mem.decorate_maps`: the same kinds, named
    const NAMED: &str = "\
7f3000400000-7f3000600000 rw-p 00000000 00:00 0 [anon: glibc: malloc arena]
Rss:                 512 kB
7f3000700000-7f3000900000 rw-p 00000000 00:00 0 [anon: glibc: pthread user stack: 4242]
Rss:                  24 kB
7f3000a00000-7f3000b00000 rw-p 00000000 00:00 0 [anon: glibc: malloc]
Rss:                 300 kB
";

    #[test]
    fn smaps_sorts_into_the_binary_the_heaps_the_stacks_and_the_rest() {
        let p = parts_of(SMAPS, "/home/runner/work/wisp/target/budgets/wisp");
        assert_eq!(p.binary, 8000);
        assert_eq!(p.files, 732, "libc and the -shm, a path with a space included");
        assert_eq!(p.heap, 3000);
        assert_eq!(p.arenas, (1, 2048), "the 64 MB-aligned mapping; its PROT_NONE rest counts nothing");
        assert_eq!(p.stacks, (2, 56), "the mapping above a one-page guard, and [stack]");
        assert_eq!(p.anon, 964);
        assert_eq!(p.anon_top, vec![900, 64]);
    }

    #[test]
    fn mappings_glibc_names_sort_as_the_unnamed_ones_do() {
        let p = parts_of(NAMED, "/w/wisp");
        assert_eq!(p.arenas, (1, 512), "named an arena, not 64 MB-aligned");
        assert_eq!(p.stacks, (1, 24), "named a stack, no guard before it");
        assert_eq!((p.anon, p.anon_top.clone()), (300, vec![300]), "a big block glibc mapped on its own");
    }

    #[test]
    fn the_line_names_every_part_or_only_the_threads_without_smaps() {
        let mut p = parts_of(SMAPS, "/home/runner/work/wisp/target/budgets/wisp");
        p.threads = Some(23);
        assert_eq!(
            p.line(),
            "binary 7.8 · other files 0.7 · [heap] 2.9 · 1 arenas 2.0 · 2 stacks 0.1 · other anon 0.9 MB (biggest 0.9, 0.1); 23 threads"
        );
        assert_eq!(Parts { threads: Some(5), ..Parts::default() }.line(), "5 threads");
    }
}
