# Target distribution estimates

Reference snapshot of where Rust binaries actually run, split into two market
segments: **server / production cloud** and **desktop / local dev / CI**. These
weights inform which cells we add to the benchmark matrix and which targets
we feature in the README chart.

Estimates as of 2026-05-14. **Conf** is a 0–100 self-assessed confidence
rating; **Range** is the plausible interval on the point estimate; **Σ** is the
running cumulative share within the segment.

## Server / production cloud

| #  | Target                          | Share  | Range       | Σ       | Conf |
|---:|---------------------------------|-------:|-------------|--------:|-----:|
|  1 | `x86_64-unknown-linux-gnu`      | 77.5 % | 72–82 %     |  77.5 % |   88 |
|  2 | `aarch64-unknown-linux-gnu`     | 14.6 % | 10–19 %     |  92.1 % |   74 |
|  3 | `x86_64-unknown-linux-musl`     |  4.2 % | 2.5–6.5 %   |  96.3 % |   68 |
|  4 | `x86_64-pc-windows-msvc`        |  2.0 % | 1.0–3.5 %   |  98.3 % |   61 |
|  5 | `aarch64-unknown-linux-musl`    |  0.9 % | 0.4–1.8 %   |  99.2 % |   55 |
|  6 | `x86_64-unknown-freebsd`        |  0.3 % | 0.1–0.7 %   |  99.5 % |   33 |
|  7 | `powerpc64le-unknown-linux-gnu` | 0.15 % | 0.05–0.35 % | 99.65 % |   25 |
|  8 | `s390x-unknown-linux-gnu`       | 0.15 % | 0.05–0.35 % | 99.80 % |   25 |
|  9 | `riscv64gc-unknown-linux-gnu`   | 0.05 % | 0.01–0.15 % | 99.85 % |   18 |
| 10 | _other native server targets_   | 0.15 % | 0.05–0.40 % | 100.0 % |   15 |

**Where you see each target:**

1. **`x86_64-unknown-linux-gnu`** — AWS EC2 Nitro x86, ECS / Fargate x86_64, EKS x86 nodes, GCP x86 VMs, Cloud Run `linux/amd64`, Azure x64 Linux VMs, Azure Container Apps `linux/amd64`, OCI Intel / AMD shapes, Alibaba x86 ECS, DigitalOcean Droplets / App Platform.
2. **`aarch64-unknown-linux-gnu`** — AWS Graviton EC2 / Lambda / ECS / EKS, GCP Axion / Ampere VMs, GKE Arm nodes, Azure Cobalt / Ampere VMs, AKS Arm64, OCI Ampere A1 / A2, OKE Arm, Alibaba YiTian ECS / ACK.
3. **`x86_64-unknown-linux-musl`** — Alpine / static x86 containers, scratch / distroless-style deployments, custom Lambda runtimes, x86 Kubernetes / ECS / GKE / AKS / OKE containers.
4. **`x86_64-pc-windows-msvc`** — Windows Server VMs, Azure Windows workloads, EC2 Windows, Windows build agents, legacy enterprise services.
5. **`aarch64-unknown-linux-musl`** — Alpine / static Arm containers on Graviton, GKE Arm, AKS Arm, OCI Ampere, Alibaba YiTian.
6. **`x86_64-unknown-freebsd`** — Niche BSD servers, custom VPS / bare-metal images, storage- and networking-heavy shops.
7. **`powerpc64le-unknown-linux-gnu`** — IBM Power Linux, enterprise / on-prem, regulated / legacy estates.
8. **`s390x-unknown-linux-gnu`** — IBM Z / LinuxONE, mainframe-adjacent enterprise workloads.
9. **`riscv64gc-unknown-linux-gnu`** — Experimental cloud, research, embedded / server dev boards.
10. **Other** — NetBSD / OpenBSD / illumos / legacy 32-bit / other specialty environments.

## Desktop / local dev / CI

| # | Target                       | Share  | Range     | Σ       | Conf |
|--:|------------------------------|-------:|-----------|--------:|-----:|
| 1 | `x86_64-pc-windows-msvc`     | 40.7 % | 35–46 %   |  40.7 % |   80 |
| 2 | `aarch64-apple-darwin`       | 24.7 % | 20–30 %   |  65.4 % |   69 |
| 3 | `x86_64-unknown-linux-gnu`   | 22.6 % | 18–28 %   |  88.0 % |   66 |
| 4 | `x86_64-apple-darwin`        |  4.3 % | 2.5–7.0 % |  92.3 % |   52 |
| 5 | `x86_64-unknown-linux-musl`  |  4.0 % | 2.5–6.0 % |  96.3 % |   55 |
| 6 | `aarch64-unknown-linux-gnu`  |  1.4 % | 0.6–3.0 % |  97.7 % |   32 |
| 7 | `aarch64-pc-windows-msvc`    |  1.3 % | 0.5–3.0 % |  99.0 % |   28 |
| 8 | `aarch64-unknown-linux-musl` |  1.0 % | 0.3–2.5 % | 100.0 % |   25 |

**Where you see each target:**

1. **`x86_64-pc-windows-msvc`** — Native Windows 10 / 11 dev machines, Visual Studio / MSVC Rust installs, GitHub Actions `windows-latest` x64, Docker Desktop Windows AMD64 hosts.
2. **`aarch64-apple-darwin`** — Apple Silicon Macs, GitHub Actions `macos-latest` / macOS Arm64 runners, native macOS developer tooling.
3. **`x86_64-unknown-linux-gnu`** — Ubuntu / Debian / Fedora / RHEL dev machines, WSL2 on x64 Windows, Linux CI, GitHub Actions `ubuntu-latest`, Docker Linux VMs on Intel hosts.
4. **`x86_64-apple-darwin`** — Intel Macs, older macOS CI, compatibility builds.
5. **`x86_64-unknown-linux-musl`** — Alpine / static local containers, CI packaging, scratch-image workflows, Docker-based dev loops.
6. **`aarch64-unknown-linux-gnu`** — Linux Arm dev machines, Arm cloud dev boxes, Arm Linux containers on Apple Silicon, WSL2 on Windows Arm.
7. **`aarch64-pc-windows-msvc`** — Windows on Arm laptops, native Arm64 Visual Studio / VS Code toolchains, GitHub Actions `windows-11-arm`.
8. **`aarch64-unknown-linux-musl`** — Alpine / static Arm containers, Apple Silicon Docker builds targeting Linux Arm64, Arm CI / package lanes.

## How we use this

1. **Benchmark cells.** A representative baseline covers every target in
   the top 5 of either segment by point estimate, plus relevant intra-target
   variations (bare-metal vs Nitro VM, Apple Silicon M1 vs M4 Pro, etc.).
   Targets below ~1 % combined market share are deferred.
2. **README chart cells.** The performance chart includes cells covering the
   top 4–5 weighted targets across both segments — roughly 90–94 % combined
   coverage by population.

## When to update

Refresh this doc when:

- Cloud providers publish new instance-generation share data.
- A new architecture crosses ~1 % combined share (Graviton 5 / Cobalt 200 /
  Axion displacement, riscv64 picking up commercial traction, etc.).
- The benchmark matrix changes in a way that diverges from these priorities.
