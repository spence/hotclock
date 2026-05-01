#[inline(always)]
#[allow(clippy::inline_always)]
pub fn ticks() -> u64 {
  #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
  {
    super::aarch64::cntvct()
  }

  #[cfg(all(
    not(any(
      target_arch = "x86_64",
      target_arch = "x86",
      target_arch = "aarch64",
      target_arch = "riscv64",
      target_arch = "powerpc64",
      target_arch = "s390x",
      target_arch = "loongarch64",
    )),
    target_os = "macos",
  ))]
  {
    super::fallback::mach_time()
  }

  #[cfg(all(
    not(any(
      target_arch = "x86_64",
      target_arch = "x86",
      target_arch = "aarch64",
      target_arch = "riscv64",
      target_arch = "powerpc64",
      target_arch = "s390x",
      target_arch = "loongarch64",
    )),
    unix,
    not(target_os = "macos"),
  ))]
  {
    super::fallback::clock_monotonic()
  }

  #[cfg(all(
    not(any(
      target_arch = "x86_64",
      target_arch = "x86",
      target_arch = "aarch64",
      target_arch = "riscv64",
      target_arch = "powerpc64",
      target_arch = "s390x",
      target_arch = "loongarch64",
    )),
    not(unix),
  ))]
  {
    super::fallback::instant_elapsed()
  }
}

#[inline]
#[must_use]
pub const fn implementation() -> &'static str {
  #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
  {
    "aarch64-cntvct"
  }

  #[cfg(all(
    not(any(
      target_arch = "x86_64",
      target_arch = "x86",
      target_arch = "aarch64",
      target_arch = "riscv64",
      target_arch = "powerpc64",
      target_arch = "s390x",
      target_arch = "loongarch64",
    )),
    target_os = "macos",
  ))]
  {
    "macos-mach"
  }

  #[cfg(all(
    not(any(
      target_arch = "x86_64",
      target_arch = "x86",
      target_arch = "aarch64",
      target_arch = "riscv64",
      target_arch = "powerpc64",
      target_arch = "s390x",
      target_arch = "loongarch64",
    )),
    unix,
    not(target_os = "macos"),
  ))]
  {
    "unix-monotonic"
  }

  #[cfg(all(
    not(any(
      target_arch = "x86_64",
      target_arch = "x86",
      target_arch = "aarch64",
      target_arch = "riscv64",
      target_arch = "powerpc64",
      target_arch = "s390x",
      target_arch = "loongarch64",
    )),
    not(unix),
  ))]
  {
    "std-instant"
  }
}
