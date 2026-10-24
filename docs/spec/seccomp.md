# Seccomp specification, version 1

Normative. This spec fills in the seccomp filter of ADR 0005, which the init installs as part of finishing the sandbox (ADR 0024). It must not change a decision recorded in an ADR; if it needs to, write a new ADR first. Where the two disagree, the ADR wins.

## Model

The filter is a **denylist**: every syscall is allowed unless it is listed here, and listed syscalls are blocked. The namespaces and Landlock do the main isolating (ADR 0005); seccomp reduces the kernel attack surface that an application can reach. We chose this over an allowlist because applications are arbitrary ELFs (ADR 0002) and an allowlist breaks them whenever a library starts using a new syscall. This is the model Docker and Flatpak use.

## Initial denylist

This is a starting point, to be reviewed against real applications during implementation, not a final list.

| Group | Syscalls |
|---|---|
| Mounts and filesystem roots | `mount`, `umount2`, `pivot_root`, `chroot`, `move_mount`, `open_tree`, `fsopen`, `fsmount`, `fsconfig`, `fspick`, `mount_setattr` |
| Kernel and boot | `kexec_load`, `kexec_file_load`, `reboot`, `init_module`, `finit_module`, `delete_module` |
| Tracing and kernel programs | `bpf`, `perf_event_open`, `ptrace`, `process_vm_readv`, `process_vm_writev` |
| Keyrings | `add_key`, `request_key`, `keyctl` |
| Clock and host identity | `settimeofday`, `clock_settime`, `clock_adjtime`, `adjtimex`, `sethostname`, `setdomainname` |
| System administration | `swapon`, `swapoff`, `acct`, `quotactl`, `iopl`, `ioperm` |
| Container escapes | `open_by_handle_at` |

Every non-native architecture is also blocked: 32-bit compatibility syscalls and the x32 ABI, so a filter on native syscalls cannot be bypassed through another syscall table.

`ptrace` is in the list but crash reporters in some applications may need it; that is one of the things the implementation review must check. `io_uring` is deliberately not in the list: Docker blocks it by default but some Rust applications use it, so it is decided during implementation testing.

## Nested user namespaces

Applications may not create new user namespaces. Chromium and Electron use them for their own sandbox, so those applications are expected to run with that sandbox off, which is acceptable because ours already encloses them (ADR 0002, `docs/checks.md`). We chose this over allowing them because a kernel bug in user-namespace handling would otherwise be reachable from inside every application, which is also what Flatpak does.

They are blocked by the sandbox's user namespace, not by a seccomp rule: as part of finishing the sandbox (ADR 0024), before it drops capabilities, the init writes `0` to `/proc/sys/user/max_user_namespaces` inside that namespace, so every later attempt to create a user namespace fails with `ENOSPC`, whichever syscall is used, `clone3` included. This was verified (`docs/checks.md`). We chose it over inspecting the flags of `unshare` and `clone` in seccomp, which cannot see the flags of `clone3` and would need `clone3` to fail with `ENOSYS`.

## Capabilities and `no_new_privs`

The application runs with an empty capability set in every set, including the bounding set, and with `no_new_privs` on, so a setuid program such as `sudo` inside the visible `/usr` gains nothing. The init drops its own capabilities after finishing the sandbox (ADR 0024). `no_new_privs` is also what lets an unprivileged process install a seccomp filter at all.

## Blocked calls

A blocked syscall returns `EPERM`. The process is not killed and nothing is logged in v1, so an application that tries something optional, such as a crash reporter using `ptrace`, can degrade instead of crashing. This applies to every entry in the denylist.
