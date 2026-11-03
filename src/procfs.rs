//! Process inspection: `/proc` on Linux, libproc on macOS.

use std::path::{Path, PathBuf};

/// `/proc/<pid>/stat` text. `None` off Linux (no procfs) or if the process is gone.
pub fn stat(pid: u32) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()
}

/// The whitespace-split fields after the last `)` (the comm may hold spaces
/// and parentheses): index 0 is stat field 3 (state), so field N is index N-3.
pub fn stat_fields(text: &str) -> Option<Vec<&str>> {
    let (_, rest) = text.rsplit_once(')')?;
    Some(rest.split_whitespace().collect())
}

/// `(comm, process group)` of `pid`.
pub fn comm_pgrp(pid: u32) -> Option<(String, i64)> {
    imp::comm_pgrp(pid)
}

/// Direct children of `pid`.
pub fn children(pid: u32) -> Vec<u32> {
    imp::children(pid)
}

/// Current working directory of `pid`.
pub fn cwd(pid: u32) -> Option<PathBuf> {
    imp::cwd(pid)
}

/// Whether `pid` has `tty` (or `/dev/tty`) open on any fd except 2.
pub fn has_tty_open(pid: u32, tty: &Path) -> bool {
    imp::has_tty_open(pid, tty)
}

#[cfg(target_os = "linux")]
mod imp {
    use super::*;

    pub fn comm_pgrp(pid: u32) -> Option<(String, i64)> {
        let s = stat(pid)?;
        let fields = stat_fields(&s)?;
        let comm = s[..s.rfind(')')?].split_once('(')?.1.to_string();
        Some((comm, fields.get(2)?.parse().ok()?))
    }

    pub fn children(pid: u32) -> Vec<u32> {
        std::fs::read_dir(format!("/proc/{pid}/task"))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|t| std::fs::read_to_string(t.path().join("children")).ok())
            .flat_map(|s| s.split_whitespace().filter_map(|p| p.parse().ok()).collect::<Vec<u32>>())
            .collect()
    }

    pub fn cwd(pid: u32) -> Option<PathBuf> {
        std::fs::read_link(format!("/proc/{pid}/cwd")).ok()
    }

    pub fn has_tty_open(pid: u32, tty: &Path) -> bool {
        std::fs::read_dir(format!("/proc/{pid}/fd")).into_iter().flatten().flatten().any(|e| {
            e.file_name() != "2" && std::fs::read_link(e.path()).is_ok_and(|t| t == tty || t == Path::new("/dev/tty"))
        })
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use std::ffi::{CStr, c_int, c_void};
    use std::mem::{size_of, zeroed};
    use std::os::unix::ffi::OsStrExt;

    /// `PROC_PIDFDVNODEPATHINFO` and `struct vnode_fdinfowithpath` from `<sys/proc_info.h>`.
    const PROC_PIDFDVNODEPATHINFO: c_int = 2;

    #[repr(C)]
    struct ProcFileInfo {
        fi_openflags: u32,
        fi_status: u32,
        fi_offset: i64,
        fi_type: i32,
        fi_guardflags: u32,
    }

    #[repr(C)]
    struct VnodeFdInfoWithPath {
        pfi: ProcFileInfo,
        pvip: libc::vnode_info_path,
    }

    /// The NUL-terminated path inside a `vnode_info_path`.
    fn vip_path(v: &libc::vnode_info_path) -> Option<PathBuf> {
        // SAFETY: `vip_path` is a plain 1024-byte char array (split in 32×32 only for old rustc).
        let bytes: &[u8] = unsafe { std::slice::from_raw_parts(v.vip_path.as_ptr().cast(), size_of_val(&v.vip_path)) };
        let c = CStr::from_bytes_until_nul(bytes).ok()?;
        (!c.is_empty()).then(|| Path::new(std::ffi::OsStr::from_bytes(c.to_bytes())).to_path_buf())
    }

    fn pidinfo<T>(pid: u32, flavor: c_int) -> Option<T> {
        // SAFETY: `T` is a plain-data libproc struct; the kernel fills at most `size_of::<T>()` bytes.
        unsafe {
            let mut out: T = zeroed();
            let n = libc::proc_pidinfo(pid as c_int, flavor, 0, (&raw mut out).cast::<c_void>(), size_of::<T>() as c_int);
            (n == size_of::<T>() as c_int).then_some(out)
        }
    }

    pub fn comm_pgrp(pid: u32) -> Option<(String, i64)> {
        let info: libc::proc_bsdinfo = pidinfo(pid, libc::PROC_PIDTBSDINFO)?;
        // SAFETY: `pbi_comm` is NUL-terminated by the kernel.
        let comm = unsafe { CStr::from_ptr(info.pbi_comm.as_ptr()) }.to_string_lossy().into_owned();
        Some((comm, i64::from(info.pbi_pgid)))
    }

    pub fn children(pid: u32) -> Vec<u32> {
        // SAFETY: the buffer holds `cap` pids; libproc never writes past `buffersize`.
        unsafe {
            // The sizing call over-reports bytes; the real one returns a pid count.
            let n = libc::proc_listchildpids(pid as c_int, std::ptr::null_mut(), 0);
            if n <= 0 {
                return vec![];
            }
            // Room for children forked since the sizing call.
            let cap = n as usize / size_of::<c_int>() + 16;
            let mut pids: Vec<c_int> = vec![0; cap];
            let n = libc::proc_listchildpids(pid as c_int, pids.as_mut_ptr().cast(), (cap * size_of::<c_int>()) as c_int);
            if n <= 0 {
                return vec![];
            }
            pids.truncate((n as usize).min(cap));
            pids.into_iter().filter(|&p| p > 0).map(|p| p as u32).collect()
        }
    }

    pub fn cwd(pid: u32) -> Option<PathBuf> {
        let info: libc::proc_vnodepathinfo = pidinfo(pid, libc::PROC_PIDVNODEPATHINFO)?;
        vip_path(&info.pvi_cdir)
    }

    pub fn has_tty_open(pid: u32, tty: &Path) -> bool {
        // SAFETY: both buffers are sized from what libproc reports; entries are plain data.
        unsafe {
            let n = libc::proc_pidinfo(pid as c_int, libc::PROC_PIDLISTFDS, 0, std::ptr::null_mut(), 0);
            if n <= 0 {
                return false;
            }
            let cap = n as usize / size_of::<libc::proc_fdinfo>() + 8;
            let mut fds: Vec<libc::proc_fdinfo> = Vec::with_capacity(cap);
            let n = libc::proc_pidinfo(
                pid as c_int,
                libc::PROC_PIDLISTFDS,
                0,
                fds.as_mut_ptr().cast(),
                (cap * size_of::<libc::proc_fdinfo>()) as c_int,
            );
            if n <= 0 {
                return false;
            }
            fds.set_len(n as usize / size_of::<libc::proc_fdinfo>());
            fds.iter().filter(|f| f.proc_fd != 2 && f.proc_fdtype == libc::PROX_FDTYPE_VNODE as u32).any(|f| {
                let mut info: VnodeFdInfoWithPath = zeroed();
                let want = size_of::<VnodeFdInfoWithPath>() as c_int;
                let got = libc::proc_pidfdinfo(
                    pid as c_int,
                    f.proc_fd,
                    PROC_PIDFDVNODEPATHINFO,
                    (&raw mut info).cast::<c_void>(),
                    want,
                );
                got == want && vip_path(&info.pvip).is_some_and(|p| p == tty || p == Path::new("/dev/tty"))
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_after_comm_with_parens_and_spaces() {
        let t = "42 (my (odd) prog) S 1 42 42 34816 42 4194304";
        let f = stat_fields(t).unwrap();
        assert_eq!(f[0], "S");
        assert_eq!(f[2], "42");
        assert_eq!(f[5], "42");
    }

    #[test]
    fn fields_none_without_paren() {
        assert!(stat_fields("garbage").is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn stat_matches_old_closure() {
        // `old` is the session.rs parser this module replaced.
        let old = |s: String| -> Option<(String, i64)> {
            let (head, rest) = s.rsplit_once(')')?;
            let comm = head.split_once('(')?.1.to_string();
            let pgrp = rest.split_whitespace().nth(2)?.parse().ok()?;
            Some((comm, pgrp))
        };
        let new = |s: &str| -> Option<(String, i64)> {
            let fields = stat_fields(s)?;
            let comm = s[..s.rfind(')')?].split_once('(')?.1.to_string();
            Some((comm, fields.get(2)?.parse().ok()?))
        };
        let own = std::fs::read_to_string("/proc/self/stat").unwrap();
        for t in [own.as_str(), "42 (my (odd) prog) S 1 42 42 34816 42 4194304", "garbage"] {
            assert_eq!(old(t.to_string()), new(t), "{t}");
        }
        assert!(new("garbage").is_none());
    }

    #[test]
    fn own_process_is_visible() {
        let me = std::process::id();
        assert_eq!(cwd(me), std::env::current_dir().unwrap().canonicalize().ok());
        let (comm, pgrp) = comm_pgrp(me).unwrap();
        assert!(!comm.is_empty());
        assert_eq!(pgrp, i64::from(unsafe { libc::getpgrp() }));
    }

    /// Between fork and exec a child still carries the test binary's name and fds.
    fn wait_exec(pid: u32) {
        for _ in 0..200 {
            if comm_pgrp(pid).is_some_and(|(c, _)| c == "sleep") {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("{pid} never became `sleep`");
    }

    #[test]
    fn child_is_listed_and_inspected() {
        let mut child = std::process::Command::new("sleep").arg("5").spawn().unwrap();
        let pid = child.id();
        wait_exec(pid);
        let listed = children(std::process::id());
        let _ = child.kill();
        let _ = child.wait();
        assert!(listed.contains(&pid), "{listed:?}");
    }

    #[test]
    fn tty_open_is_seen_on_fd0_but_not_stderr_only() {
        use std::os::fd::FromRawFd;
        use std::process::{Command, Stdio};
        let (mut master, mut slave) = (0, 0);
        let mut name = [0 as libc::c_char; 128];
        // SAFETY: plain openpty; `name` is large enough for a tty path.
        let ok = unsafe { libc::openpty(&mut master, &mut slave, name.as_mut_ptr(), std::ptr::null_mut(), std::ptr::null_mut()) };
        assert_eq!(ok, 0);
        // openpty fds are inheritable; keep them out of the children except via stdin/stderr.
        for fd in [master, slave] {
            // SAFETY: valid fds just returned by openpty.
            unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) };
        }
        // SAFETY: both fds were just opened and are owned here.
        let (master, slave) = unsafe { (std::fs::File::from_raw_fd(master), std::fs::File::from_raw_fd(slave)) };
        let tty = PathBuf::from(unsafe { std::ffi::CStr::from_ptr(name.as_ptr()) }.to_str().unwrap());

        let on_stdin = Command::new("sleep").arg("5").stdin(slave.try_clone().unwrap()).spawn().unwrap();
        let on_stderr = Command::new("sleep")
            .arg("5")
            .stdin(Stdio::null())
            .stderr(slave.try_clone().unwrap())
            .spawn()
            .unwrap();
        let mut kids = [on_stdin, on_stderr];
        // Before exec both children still hold every fd we had open.
        kids.iter().for_each(|k| wait_exec(k.id()));
        let seen: Vec<bool> = kids.iter().map(|k| has_tty_open(k.id(), &tty)).collect();
        for k in &mut kids {
            let _ = k.kill();
            let _ = k.wait();
        }
        drop((master, slave));
        assert_eq!(seen, [true, false]);
    }
}
