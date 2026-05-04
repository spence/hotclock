# Runtime Selection Proof

This branch benchmarks raw clock candidates across same-target environments and
records where runtime selection changes the fastest usable clock.

## First-party blocked-side results

GitHub-hosted x86_64 Linux runners are a negative control for PMU/RDPMC. The
target is `x86_64-unknown-linux-gnu`, but Azure does not expose
`/sys/bus/event_source/devices/cpu/rdpmc` or `/sys/devices/cpu/rdpmc`, so every
RDPMC candidate fails and `rdtsc` wins.

| Environment                                    | perf_event_paranoid | rdpmc sysfs | RDPMC fixed | perf RDPMC | RDTSC ns/op | MONOTONIC ns/op |
|------------------------------------------------|---------------------|-------------|-------------|------------|------------:|----------------:|
| linux-amd64-host-default-perf-policy           | 4                   | missing     | failed      | failed     |      11.238 |          29.020 |
| linux-amd64-host-opened-perf-policy            | 2                   | missing     | failed      | failed     |      11.237 |          28.964 |
| linux-amd64-docker-default-seccomp-perf-policy | 4                   | missing     | failed      | failed     |      11.198 |          28.961 |
| linux-amd64-docker-privileged-perf-policy      | 2                   | missing     | failed      | failed     |      11.202 |          29.015 |

Artifact: <https://github.com/spence/hotclock/actions/runs/25301523449>

The same benchmark also ran on `poly`, an x86_64 Linux EC2 `t3.micro` VM. Even
with `perf_event_paranoid` lowered to 2, the guest has no exported CPU PMU
device, so RDPMC remains unavailable and `rdtsc` wins.

| Environment                         | perf_event_paranoid | CPU PMU device | RDPMC fixed | perf RDPMC | RDTSC ns/op | MONOTONIC ns/op |
|-------------------------------------|---------------------|----------------|-------------|------------|------------:|----------------:|
| poly-aws-x86_64-vm                  | 4                   | missing        | failed      | failed     |       9.493 |          23.443 |
| poly-aws-x86_64-vm-opened-paranoid  | 2                   | missing        | failed      | failed     |       9.445 |          23.423 |

## AWS same-target clock flip

On 2026-05-04, the benchmark ran on five short-lived EC2 instances in
`us-east-1`. Each instance was launched from Ubuntu 24.04, had
`perf_event_paranoid` lowered to 2, and was terminated after the report was
copied back. Raw reports are stored in `benches/aws-runtime-selection-2026-05-04`.

| AWS target | Architecture | CPU / platform | Fastest clock | PMU / RDPMC result | Other clocks |
|------------|--------------|----------------|---------------|--------------------|--------------|
| t3.micro | x86_64 | Intel Xeon Platinum 8259CL | x86_64-rdtsc, 9.423 ns/op | direct RDPMC failed; perf RDPMC failed | unix-monotonic 23.419 ns/op |
| m7i.large | x86_64 | Intel Xeon Platinum 8488C | x86_64-rdtsc, 13.475 ns/op | direct RDPMC failed; perf RDPMC 1310.988 ns/op | unix-monotonic 23.484 ns/op |
| m7a.large | x86_64 | AMD EPYC 9R14 | x86_64-rdtsc, 9.552 ns/op | direct RDPMC failed; perf RDPMC 976.080 ns/op | unix-monotonic 24.011 ns/op |
| m5zn.6xlarge | x86_64 | Intel Xeon Platinum 8252C | x86_64-rdtsc, 6.482 ns/op | direct RDPMC failed; perf RDPMC 686.962 ns/op | unix-monotonic 16.096 ns/op |
| c7g.large | aarch64 | AWS Graviton3 | aarch64-cntvct, 6.692 ns/op | not an x86 RDPMC target | unix-monotonic 31.509 ns/op |

These AWS results did not produce an RDPMC-over-RDTSC win. They did produce a
different selection-critical proof: on common AWS VMs, a counter path can be
present and still be unusably slow. `m7i.large`, `m7a.large`, and
`m5zn.6xlarge` all allowed a perf-backed counter read after the kernel policy
was opened, but the measured cost was hundreds to thousands of nanoseconds per
read. The selector must validate read cost, not just candidate availability.

The same day, the benchmark ran again with a child-isolated direct RDPMC
candidate. The direct candidate intentionally does not rely on reading the Linux
`rdpmc` sysfs knob, because several EC2 metal kernels allow root to write the
knob but do not allow the benchmark user to read it back. If RDPMC is illegal,
the candidate process crashes and the parent records a failed clock.

Raw reports are stored in `benches/aws-runtime-selection-2026-05-04-raw-rdpmc`.

| AWS target | Rust target | Environment | Fastest clock | Other clocks |
|------------|-------------|-------------|---------------|--------------|
| t3.micro | x86_64-unknown-linux-musl | Nitro VM | x86_64-rdtsc, 9.432 ns/op | direct RDPMC failed; perf RDPMC failed; unix-monotonic 23.504 ns/op |
| c5n.metal | x86_64-unknown-linux-musl | EC2 bare metal | x86_64-rdtsc, 7.228 ns/op | direct RDPMC 7.673 ns/op; perf RDPMC 11.174 ns/op; unix-monotonic 18.609 ns/op |
| c5.metal | x86_64-unknown-linux-musl | EC2 bare metal | x86_64-rdpmc-fixed-core-cycles, 5.291 ns/op | rdtsc 6.430 ns/op; perf RDPMC 8.219 ns/op; unix-monotonic 16.689 ns/op |
| m5zn.metal | x86_64-unknown-linux-musl | EC2 bare metal | x86_64-rdpmc-fixed-core-cycles, 4.228 ns/op | rdtsc 5.577 ns/op; perf RDPMC 7.118 ns/op; unix-monotonic 14.470 ns/op |
| m7i.metal-24xl | x86_64-unknown-linux-musl | EC2 bare metal | x86_64-rdpmc-fixed-core-cycles, 3.163 ns/op | perf RDPMC 4.889 ns/op; rdtsc 6.851 ns/op; unix-monotonic 14.522 ns/op |
| c7i.metal-24xl | x86_64-unknown-linux-musl | EC2 bare metal | x86_64-rdpmc-fixed-core-cycles, 3.164 ns/op | perf RDPMC 5.003 ns/op; rdtsc 6.851 ns/op; unix-monotonic 14.592 ns/op |

This is the concrete proof point: the same Rust target flips winners across
modern AWS environments. A Nitro VM chooses `rdtsc` because direct RDPMC faults.
Several EC2 metal environments choose direct fixed-counter RDPMC because it is
both available and faster than `rdtsc`.

## AWS Lambda sweep

The same benchmark also ran in Lambda custom runtimes on 2026-05-04. Raw
reports are stored in `benches/aws-runtime-selection-2026-05-04-lambda`.

| AWS target | Rust target | Environment | Fastest clock | Other clocks |
|------------|-------------|-------------|---------------|--------------|
| Lambda x86_64 custom runtime | x86_64-unknown-linux-musl | Lambda managed runtime | x86_64-rdtsc, 9.153 ns/op | direct RDPMC failed; perf RDPMC failed; unix-monotonic 26.462 ns/op |
| Lambda arm64 custom runtime | aarch64-unknown-linux-musl | Lambda managed runtime | aarch64-cntvct, 6.667 ns/op | unix-monotonic 34.900 ns/op |

Lambda behaves like the VM side of the x86_64 split: direct RDPMC faults and
`rdtsc` wins. On arm64 Lambda, `cntvct` is the clear winner.

## AWS target matrix

The target matrix keeps Rust target triples separate from runtime environment.
Raw reports are stored in `benches/aws-target-matrix-2026-05-04`.

| Rust target | Environment | AWS runtime | Fastest clock | Candidate timings |
|-------------|-------------|-------------|---------------|-------------------|
| x86_64-unknown-linux-musl | Lambda custom runtime | Lambda managed microVM | x86_64-rdtsc, 9.153 ns/op | direct RDPMC failed; unix-monotonic 26.462 ns/op |
| x86_64-unknown-linux-musl | t3.micro | Nitro VM | x86_64-rdtsc, 9.432 ns/op | direct RDPMC failed; unix-monotonic 23.398 ns/op |
| x86_64-unknown-linux-musl | m7i.metal-24xl | EC2 bare metal | x86_64-rdpmc-fixed-core-cycles, 3.163 ns/op | rdtsc 6.849 ns/op; unix-monotonic 14.686 ns/op |
| x86_64-unknown-linux-gnu | t3.micro | Nitro VM | x86_64-rdtsc, 9.423 ns/op | direct RDPMC failed; unix-monotonic 23.384 ns/op |
| x86_64-unknown-linux-gnu | m7i.metal-24xl | EC2 bare metal | x86_64-rdpmc-fixed-core-cycles, 2.901 ns/op | rdtsc 6.852 ns/op; unix-monotonic 14.224 ns/op |
| aarch64-unknown-linux-musl | Lambda custom runtime | Lambda managed microVM | aarch64-cntvct, 6.667 ns/op | unix-monotonic 34.900 ns/op |
| aarch64-unknown-linux-musl | t4g.micro | Nitro VM | aarch64-cntvct, 7.307 ns/op | pmccntr_el0 failed; unix-monotonic 32.849 ns/op |
| aarch64-unknown-linux-musl | c7g.metal | EC2 bare metal | aarch64-cntvct, 6.675 ns/op | pmccntr_el0 failed; unix-monotonic 30.579 ns/op |
| aarch64-unknown-linux-gnu | t4g.micro | Nitro VM | aarch64-cntvct, 7.307 ns/op | pmccntr_el0 failed; unix-monotonic 32.436 ns/op |
| aarch64-unknown-linux-gnu | c7g.metal | EC2 bare metal | aarch64-cntvct, 6.676 ns/op | pmccntr_el0 failed; unix-monotonic 31.102 ns/op |

This adds a second strict same-target proof point:
`x86_64-unknown-linux-gnu` flips from `rdtsc` on a Nitro VM to direct fixed
counter RDPMC on EC2 bare metal. The `x86_64-unknown-linux-musl` target also
has two runtime-authority splits: Lambda and t3.micro both choose `rdtsc`, while
m7i.metal chooses direct RDPMC.

The aarch64 AWS targets are negative controls. On the tested Graviton Lambda,
KVM, and bare-metal environments, `cntvct_el0` remains the fastest usable clock
and `pmccntr_el0` is not directly executable by the benchmark process.

## AWS four-target proof

The four-target matrix reran the x86 family with fresh binaries and added the
32-bit Linux targets. Raw reports are stored in
`benches/aws-four-target-matrix-2026-05-04`.

| Rust target | Lambda | KVM / Nitro VM | EC2 bare metal | Proof |
|-------------|--------|----------------|----------------|-------|
| x86_64-unknown-linux-musl | rdtsc, 9.506 ns/op | rdtsc, 9.426 ns/op | rdpmc fixed, 2.907 ns/op | Lambda/KVM choose rdtsc; metal chooses rdpmc |
| x86_64-unknown-linux-gnu | rdtsc, 9.527 ns/op | rdtsc, 9.424 ns/op | rdpmc fixed, 2.902 ns/op | Lambda/KVM choose rdtsc; metal chooses rdpmc |
| i686-unknown-linux-musl | not executable in Lambda | rdtsc, 9.751 ns/op | rdpmc fixed, 3.162 ns/op | KVM chooses rdtsc; metal chooses rdpmc |
| i686-unknown-linux-gnu | not executable in Lambda | rdtsc, 9.426 ns/op | rdpmc fixed, 2.902 ns/op | KVM chooses rdtsc; metal chooses rdpmc |

This gives four distinct Rust targets with measured runtime clock flips. The
target triple does not determine the fastest clock. Runtime authority does:
Lambda and Nitro VMs block direct RDPMC, while EC2 metal exposes it and it beats
RDTSC by roughly 2x on the tested m7i.metal host.

The i686 Lambda rows are recorded as negative execution checks. Lambda's x86_64
custom runtime did not execute the 32-bit binaries, so the i686 proof uses the
KVM-vs-metal pair.

## AWS full clock and crate benchmarks

The full benchmark matrix reran every unique target/environment from the
four-target proof with all benchmarked hotclock candidates plus `std::time`,
`quanta@0.12.6`, `minstant@0.1.7`, and `fastant@0.1.11`.

Raw JSON reports and the complete ns/op table are stored in
`benches/aws-full-clock-benchmarks-2026-05-04`.

## Published proof pairs

The same apparent Rust target can still have different fastest usable clocks.
The libcpucycles machine matrix shows the relevant runtime split directly. The
candidate evidence below uses libcpucycles precision values; lower is faster,
and precision `0` means the candidate was not usable.

### x86_64 Linux: PMU/RDPMC vs RDTSC

`amd64-perfpmcff` and `amd64-perfpmc` require Linux `perf_event`,
`perf_event_paranoid <= 2`, and RDPMC sysfs permission. `amd64-tsc` only needs
RDTSC, which is normally enabled.

| Machine | Runtime condition | Fastest usable clock | Candidate evidence |
|---------|-------------------|----------------------|--------------------|
| titan0, Intel Xeon E3-1275 V3 | Intel RDPMC path available | amd64-perfpmcff | amd64-perfpmcff precision 37, amd64-tsc precision 144 |
| phoenix, AMD Ryzen 5 7640HS | AMD perf RDPMC path available | amd64-perfpmc | amd64-perfpmc precision 33, amd64-tsc precision 111 |
| cfarm14, Intel Xeon E5-2620 v3, Debian 12 Linux | PMU/RDPMC candidates unavailable | amd64-tsc | perf/RDPMC candidates precision 0, amd64-tsc precision 118 |
| saber214, AMD FX-8350 | PMU/RDPMC candidates unavailable | amd64-tsc | perf/RDPMC candidates precision 0, amd64-tsc precision 171 |

Sources:

- <https://cpucycles.cr.yp.to/counters.html>
- <https://man7.org/linux/man-pages/man2/perf_event_open.2.html>
- <https://docs.docker.com/engine/security/seccomp/>

### aarch64 Linux: PMU counter vs CNTVCT

`arm64-perfpmc` depends on Linux perf/user PMU access. `arm64-vct` reads the
architectural virtual counter and remains available when PMU access is blocked.

| Machine | Runtime condition | Fastest usable clock | Candidate evidence |
|---------|-------------------|----------------------|--------------------|
| pi3aplus, Broadcom BCM2837B0 | PMU counter available | arm64-perfpmc | arm64-perfpmc precision 8, arm64-vct precision 173 |
| pi5, Broadcom BCM2712 | PMU counter available | arm64-perfpmc | arm64-perfpmc precision 4, arm64-vct precision 144 |
| cfarm185, Ampere eMAG 8180, AlmaLinux 8.10 Linux | PMU counter unavailable | arm64-vct | arm64-perfpmc precision 0, arm64-vct precision 175 |

Source: <https://cpucycles.cr.yp.to/counters.html>

## Conclusion

Runtime selection is justified for Linux targets where the fastest candidate is
controlled by runtime authority: kernel PMU policy, RDPMC sysfs state,
container seccomp policy, capabilities, and hypervisor exposure. Compile-time
target metadata can reduce the candidate set, but it cannot know whether PMU or
RDPMC will be usable in the process that actually runs the binary.
