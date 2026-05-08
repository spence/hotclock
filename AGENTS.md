# tach Agent Rules

## Benchmarking Standard

- Benchmark data for a target/environment combination must include the public `tach` selected APIs, the active comparison crates, and every clock primitive available for that architecture.
- Do not benchmark only clocks used by current selection. Include implemented architecture clocks that selection does not currently choose from, so the data proves whether selection is complete.
- Benchmark `tach::Instant::now()` and `tach::Cycles::now()` through the public API, then benchmark each direct clock primitive so selected API cost can be compared against the chosen primitive itself.
- Benchmark every crate currently used in README or release comparison data for the same target/environment run.
- Record clocks that are unavailable, permission-blocked, unsupported, panicking, or faulting as explicit results with the reason. Do not omit them silently.
- A benchmark run must prove two things: `tach` selected the fastest eligible clock, and the selected API cost tracks the direct primitive cost closely enough to support the inlined-performance claim.
- Record the target triple, environment, hypervisor/container/bare-metal class, CPU model, OS/kernel version, Rust version, sample settings, and benchmark command alongside the results.
