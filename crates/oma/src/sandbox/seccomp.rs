//! The seccomp filter of `docs/spec/seccomp.md`: a denylist. Every syscall is allowed unless
//! listed, and listed ones fail with `EPERM`; so does every non-x86_64 architecture and every
//! x32 syscall number, so another syscall table cannot bypass the list.

use std::io;

const AUDIT_ARCH_X86_64: u32 = 0xc000_003e;
/// Set in the syscall number of the x32 ABI.
const X32_SYSCALL_BIT: u32 = 0x4000_0000;

// Classic BPF opcodes.
const BPF_LD_W_ABS: u16 = 0x20;
const BPF_JMP_JEQ_K: u16 = 0x15;
const BPF_JMP_JSET_K: u16 = 0x45;
const BPF_RET_K: u16 = 0x06;

const SECCOMP_RET_ALLOW: u32 = 0x7fff_0000;
const SECCOMP_RET_ERRNO: u32 = 0x0005_0000;

const OFFSET_NR: u32 = 0;
const OFFSET_ARCH: u32 = 4;

/// The initial denylist of `docs/spec/seccomp.md`. Nested user namespaces are not here: the
/// init blocks them with a sysctl instead.
pub const DENIED: &[libc::c_long] = &[
    // Mounts and filesystem roots
    libc::SYS_mount,
    libc::SYS_umount2,
    libc::SYS_pivot_root,
    libc::SYS_chroot,
    libc::SYS_move_mount,
    libc::SYS_open_tree,
    libc::SYS_fsopen,
    libc::SYS_fsmount,
    libc::SYS_fsconfig,
    libc::SYS_fspick,
    libc::SYS_mount_setattr,
    // Kernel and boot
    libc::SYS_kexec_load,
    libc::SYS_kexec_file_load,
    libc::SYS_reboot,
    libc::SYS_init_module,
    libc::SYS_finit_module,
    libc::SYS_delete_module,
    // Tracing and kernel programs
    libc::SYS_bpf,
    libc::SYS_perf_event_open,
    libc::SYS_ptrace,
    libc::SYS_process_vm_readv,
    libc::SYS_process_vm_writev,
    // Keyrings
    libc::SYS_add_key,
    libc::SYS_request_key,
    libc::SYS_keyctl,
    // Clock and host identity
    libc::SYS_settimeofday,
    libc::SYS_clock_settime,
    libc::SYS_clock_adjtime,
    libc::SYS_adjtimex,
    libc::SYS_sethostname,
    libc::SYS_setdomainname,
    // System administration
    libc::SYS_swapon,
    libc::SYS_swapoff,
    libc::SYS_acct,
    libc::SYS_quotactl,
    libc::SYS_iopl,
    libc::SYS_ioperm,
    // Container escapes
    libc::SYS_open_by_handle_at,
];

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Insn {
    code: u16,
    jt: u8,
    jf: u8,
    k: u32,
}

#[repr(C)]
struct Program {
    len: u16,
    filter: *const Insn,
}

fn insn(code: u16, jt: u8, jf: u8, k: u32) -> Insn {
    Insn { code, jt, jf, k }
}

/// The filter program for `denied`.
fn program(denied: &[libc::c_long]) -> Vec<Insn> {
    // Layout: arch check (3), load nr, x32 test, one test per syscall, then ALLOW and DENY.
    let tests = denied.len();
    let mut p = Vec::with_capacity(6 + tests + 2);
    p.push(insn(BPF_LD_W_ABS, 0, 0, OFFSET_ARCH));
    p.push(insn(BPF_JMP_JEQ_K, 1, 0, AUDIT_ARCH_X86_64));
    p.push(insn(
        BPF_RET_K,
        0,
        0,
        SECCOMP_RET_ERRNO | libc::EPERM as u32,
    ));
    p.push(insn(BPF_LD_W_ABS, 0, 0, OFFSET_NR));
    // From the instruction after the test at index i, DENY is `tests - (i - 4)` further on
    // after ALLOW: the remaining tests, then ALLOW itself.
    let to_deny = |index: usize| -> u8 {
        let remaining_tests = 4 + 1 + tests - (index + 1);
        u8::try_from(remaining_tests + 1).expect("denylist too long for short jumps")
    };
    p.push(insn(BPF_JMP_JSET_K, to_deny(4), 0, X32_SYSCALL_BIT));
    for (i, nr) in denied.iter().enumerate() {
        p.push(insn(BPF_JMP_JEQ_K, to_deny(5 + i), 0, *nr as u32));
    }
    p.push(insn(BPF_RET_K, 0, 0, SECCOMP_RET_ALLOW));
    p.push(insn(
        BPF_RET_K,
        0,
        0,
        SECCOMP_RET_ERRNO | libc::EPERM as u32,
    ));
    p
}

/// Installs the filter on the calling thread and every thread and child it creates later.
/// Sets `no_new_privs` first, which installing a filter without privilege requires.
pub fn install() -> io::Result<()> {
    install_denylist(DENIED)
}

fn install_denylist(denied: &[libc::c_long]) -> io::Result<()> {
    let filter = program(denied);
    let prog = Program {
        len: u16::try_from(filter.len()).expect("filter fits"),
        filter: filter.as_ptr(),
    };
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let r = unsafe {
        libc::syscall(
            libc::SYS_seccomp,
            1u32, /* SECCOMP_SET_MODE_FILTER */
            0u32,
            &prog as *const Program,
        )
    };
    if r != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::process::Command;

    use super::*;
    use crate::testutil::{in_child, is_child};

    #[test]
    fn the_denylist_has_no_duplicates_and_every_number_fits() {
        let unique: HashSet<_> = DENIED.iter().collect();
        assert_eq!(unique.len(), DENIED.len());
        assert!(
            DENIED
                .iter()
                .all(|n| *n >= 0 && (*n as u32) < X32_SYSCALL_BIT)
        );
    }

    #[test]
    fn the_program_ends_in_allow_then_deny_and_jumps_land_on_deny() {
        let denied = [10, 20, 30];
        let p = program(&denied);
        let deny = p.len() - 1;
        assert_eq!(p[p.len() - 2].k, SECCOMP_RET_ALLOW);
        assert_eq!(p[deny].k, SECCOMP_RET_ERRNO | libc::EPERM as u32);
        // The x32 test and every syscall test jump to DENY when they match.
        for (index, instruction) in p.iter().enumerate().take(p.len() - 2).skip(4) {
            assert_eq!(
                index + 1 + instruction.jt as usize,
                deny,
                "instruction {index}"
            );
            assert_eq!(instruction.jf, 0);
        }
    }

    fn errno_of(nr: libc::c_long) -> Option<i32> {
        let r = unsafe { libc::syscall(nr, 0, 0, 0, 0, 0, 0) };
        (r == -1).then(|| io::Error::last_os_error().raw_os_error().unwrap())
    }

    #[test]
    fn listed_calls_fail_with_eperm_and_others_still_work() {
        in_child("sandbox::seccomp::tests::listed_calls_fail_with_eperm_and_others_still_work");
        if !is_child() {
            return;
        }
        // A null argument fails with EFAULT until the filter is installed, so EPERM afterwards
        // can only come from the filter. (PTRACE_TRACEME would hang the child.)
        assert_eq!(errno_of(libc::SYS_adjtimex), Some(libc::EFAULT));

        install().unwrap();

        assert_eq!(errno_of(libc::SYS_ptrace), Some(libc::EPERM));
        assert_eq!(errno_of(libc::SYS_keyctl), Some(libc::EPERM));
        assert_eq!(errno_of(libc::SYS_adjtimex), Some(libc::EPERM));
        assert_eq!(errno_of(libc::SYS_mount), Some(libc::EPERM));
        assert_eq!(errno_of(libc::SYS_open_by_handle_at), Some(libc::EPERM));
        assert_eq!(errno_of(libc::SYS_bpf), Some(libc::EPERM));

        // Allowed calls are untouched.
        assert!(unsafe { libc::getpid() } > 0);
        assert!(std::fs::read_to_string("/proc/self/comm").is_ok());
        let t = unsafe { libc::time(std::ptr::null_mut()) };
        assert!(t > 0);

        // The x32 ABI is another syscall table: any number with its bit set is refused.
        let x32_getpid = (X32_SYSCALL_BIT as libc::c_long) | libc::SYS_getpid;
        assert_eq!(errno_of(x32_getpid), Some(libc::EPERM));
    }

    #[test]
    fn the_filter_survives_a_fork_and_threads() {
        in_child("sandbox::seccomp::tests::the_filter_survives_a_fork_and_threads");
        if !is_child() {
            return;
        }
        install().unwrap();
        let from_thread = std::thread::spawn(|| errno_of(libc::SYS_ptrace))
            .join()
            .unwrap();
        assert_eq!(from_thread, Some(libc::EPERM));
        let status = Command::new("/usr/bin/true").status().unwrap();
        assert!(
            status.success(),
            "ordinary programs still run under the filter"
        );
    }
}
