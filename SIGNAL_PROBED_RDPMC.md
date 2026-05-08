# Signal-Probed Direct `RDPMC` For `Cycles`

Status: deferred.

This plan exists because direct `RDPMC` can be the fastest `Cycles` source on Linux x86
servers, but the production win is narrow. The current selector already uses the safe paths:
sysfs-approved direct `RDPMC`, perf-mmap `RDPMC`, `RDTSC`, then the OS monotonic fallback.
The missing path is blind direct `RDPMC` on machines where the instruction works but the
kernel does not expose a clean permission signal.

### Goal

Use direct `RDPMC` for `Cycles` when it is the fastest working counter, without spawning a
child process and without allowing unsupported machines to panic, segfault, or abort.

### Non-Goals

- Do not use this path for `Instant`; `Instant` remains an elapsed-time clock.
- Do not spawn a helper process for detection.
- Do not install a process-global permanent fault handler in the first version.
- Do not add dependencies just to wrap Unix signal APIs.

### Target Scope

Start with Linux x86:

- `x86_64-unknown-linux-gnu`
- `x86_64-unknown-linux-musl`
- `i686-unknown-linux-gnu`
- `i686-unknown-linux-musl`

The payoff is concentrated in Linux `x86_64` bare-metal and metal-cloud environments. Managed
VMs, serverless runtimes, and locked-down containers normally block direct PMU access and fall
back to `RDTSC`.

### Design

Add a private Linux x86 probe that temporarily installs `SIGSEGV` and `SIGILL` handlers during
cycle-counter selection.

The probe should:

- Guard installation with one process-wide lock.
- Mark the probing thread with thread-local state.
- Execute a single direct `RDPMC` fixed-counter read.
- Recover only when the fault comes from the known probe instruction on the probing thread.
- Restore the previous handlers before returning.
- Chain or restore default behavior for every unrelated signal.
- Validate the successful counter by checking advancement and measured latency.

If the probe succeeds, the `Cycles` selector can benchmark direct `RDPMC` against perf-mmap
`RDPMC`, `RDTSC`, and the fallback. If the probe fails, selection continues without crashing.

### Implementation Steps

1. Add `src/arch/linux_signal_probe_x86.rs` behind Linux `x86`/`x86_64` cfgs.
2. Define raw FFI for `sigaction`, signal sets, and the platform `ucontext` register fields we
   need.
3. Add a tiny probe function with a stable instruction label around `rdpmc`.
4. In the handler, detect the probe fault and advance `RIP`/`EIP` past the two-byte `RDPMC`
   instruction.
5. Add `signal_probed_direct_rdpmc_fixed_core_cycles_available()`.
6. Update the direct `RDPMC` candidate prepare function to accept sysfs permission or the
   signal-probed result.
7. Extend `HOTCLOCK_SELECTOR_TRACE=1` output so the trace reports whether direct `RDPMC` was
   enabled by sysfs or by signal probing.
8. Add selector tests that prove a failed probe falls through to `RDTSC` without aborting.
9. Add AWS validation rows for t3/Lambda, c5/m7i metal, and local Linux.

### Validation Matrix

Required before enabling by default:

- t3/KVM Linux x86_64: probe fails cleanly, `Cycles` selects `RDTSC`.
- Lambda Linux x86_64: probe fails cleanly, `Cycles` selects `RDTSC`.
- c5/m7i metal Linux x86_64: probe selects direct `RDPMC` only when direct `RDPMC` benchmarks
  faster than perf-mmap `RDPMC` and `RDTSC`.
- Docker default seccomp on x86_64: probe fails cleanly, no process crash.
- Existing i686 validation: probe either selects direct `RDPMC` or falls back cleanly.
- `cargo test` and the selection validation runner pass with and without selector tracing.

### Acceptance Criteria

- `Cycles::now()` never panics, segfaults, or aborts during detection.
- Unsupported environments continue selecting `RDTSC` or the OS fallback.
- Supported direct-`RDPMC` environments patch warmed `Cycles` callsites to the direct path.
- Benchmarks show direct `RDPMC` beating the selected non-direct alternative by at least 20% on
  one production-relevant x86_64 metal environment.
- The implementation preserves the zero-dependency crate contract.

### Risks

- Unix signal handlers are process-global. The handler must be temporary, guarded, and exact.
- `ucontext` register layouts differ across x86_64/i686 and libc targets.
- A successful startup probe does not prove the host cannot revoke access later.
- A permanent downgrade-on-fault handler would solve later revocation, but it is more invasive
  and remains out of scope for the first version.
