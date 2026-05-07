#[cfg(all(target_os = "macos", not(target_arch = "aarch64")))]
mod mach {
  unsafe extern "C" {
    fn mach_absolute_time() -> u64;
  }

  #[inline(always)]
  pub fn mach_time() -> u64 {
    // SAFETY: `mach_absolute_time` takes no arguments, has no Rust-side aliasing
    // requirements, and returns the host monotonic tick value.
    unsafe { mach_absolute_time() }
  }
}

#[cfg(all(target_os = "macos", not(target_arch = "aarch64")))]
pub use mach::*;

#[cfg(all(unix, not(target_os = "macos")))]
mod monotonic {
  #[cfg(all(
    feature = "bench-internals",
    target_os = "linux",
    any(
      target_arch = "x86",
      target_arch = "x86_64",
      target_arch = "aarch64",
      target_arch = "riscv64",
      target_arch = "powerpc64",
      target_arch = "s390x",
      target_arch = "loongarch64",
    )
  ))]
  use std::ffi::c_long;
  #[cfg(all(feature = "bench-internals", target_os = "linux", target_env = "gnu"))]
  use std::ffi::{c_char, c_void};
  #[cfg(all(feature = "bench-internals", target_os = "linux", target_env = "gnu"))]
  use std::sync::OnceLock;

  #[cfg(all(target_pointer_width = "32", not(target_env = "musl")))]
  type TimeField = i32;
  #[cfg(any(target_pointer_width = "64", target_env = "musl"))]
  type TimeField = i64;

  #[repr(C)]
  struct Timespec {
    tv_sec: TimeField,
    tv_nsec: TimeField,
  }

  const CLOCK_MONOTONIC: i32 = 1;
  #[cfg(all(feature = "bench-internals", target_os = "linux"))]
  const CLOCK_MONOTONIC_RAW: i32 = 4;
  #[cfg(all(feature = "bench-internals", target_os = "linux"))]
  const CLOCK_BOOTTIME: i32 = 7;

  #[cfg(all(feature = "bench-internals", target_os = "linux", target_arch = "x86"))]
  const SYS_CLOCK_GETTIME: c_long = 265;
  #[cfg(all(feature = "bench-internals", target_os = "linux", target_arch = "x86_64"))]
  const SYS_CLOCK_GETTIME: c_long = 228;
  #[cfg(all(feature = "bench-internals", target_os = "linux", target_arch = "aarch64"))]
  const SYS_CLOCK_GETTIME: c_long = 113;
  #[cfg(all(feature = "bench-internals", target_os = "linux", target_arch = "riscv64"))]
  const SYS_CLOCK_GETTIME: c_long = 113;
  #[cfg(all(feature = "bench-internals", target_os = "linux", target_arch = "loongarch64"))]
  const SYS_CLOCK_GETTIME: c_long = 113;
  #[cfg(all(feature = "bench-internals", target_os = "linux", target_arch = "powerpc64"))]
  const SYS_CLOCK_GETTIME: c_long = 246;
  #[cfg(all(feature = "bench-internals", target_os = "linux", target_arch = "s390x"))]
  const SYS_CLOCK_GETTIME: c_long = 260;

  unsafe extern "C" {
    fn clock_gettime(clk_id: i32, tp: *mut Timespec) -> i32;
  }

  #[inline(always)]
  pub fn clock_monotonic() -> u64 {
    read_clock(CLOCK_MONOTONIC)
  }

  #[inline(always)]
  #[cfg(all(feature = "bench-internals", target_os = "linux"))]
  pub fn clock_monotonic_raw() -> u64 {
    read_clock(CLOCK_MONOTONIC_RAW)
  }

  #[inline(always)]
  #[cfg(all(feature = "bench-internals", target_os = "linux"))]
  pub fn clock_boottime() -> u64 {
    read_clock(CLOCK_BOOTTIME)
  }

  #[inline(always)]
  fn read_clock(clock_id: i32) -> u64 {
    let mut ts = Timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: `ts` is a valid, writable `timespec` pointer for the duration of the call.
    let rc = unsafe { clock_gettime(clock_id, &mut ts) };
    debug_assert_eq!(rc, 0);
    timespec_to_nanos(ts)
  }

  #[inline(always)]
  fn timespec_to_nanos(ts: Timespec) -> u64 {
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
  }

  #[cfg(all(
    feature = "bench-internals",
    target_os = "linux",
    any(
      target_arch = "x86",
      target_arch = "x86_64",
      target_arch = "aarch64",
      target_arch = "riscv64",
      target_arch = "powerpc64",
      target_arch = "s390x",
      target_arch = "loongarch64",
    )
  ))]
  unsafe extern "C" {
    fn syscall(number: c_long, ...) -> c_long;
  }

  #[inline(always)]
  #[cfg(all(
    feature = "bench-internals",
    target_os = "linux",
    any(
      target_arch = "x86",
      target_arch = "x86_64",
      target_arch = "aarch64",
      target_arch = "riscv64",
      target_arch = "powerpc64",
      target_arch = "s390x",
      target_arch = "loongarch64",
    )
  ))]
  pub fn syscall_clock_monotonic() -> u64 {
    read_clock_syscall(CLOCK_MONOTONIC)
  }

  #[inline(always)]
  #[cfg(all(
    feature = "bench-internals",
    target_os = "linux",
    any(
      target_arch = "x86",
      target_arch = "x86_64",
      target_arch = "aarch64",
      target_arch = "riscv64",
      target_arch = "powerpc64",
      target_arch = "s390x",
      target_arch = "loongarch64",
    )
  ))]
  pub fn syscall_clock_monotonic_raw() -> u64 {
    read_clock_syscall(CLOCK_MONOTONIC_RAW)
  }

  #[inline(always)]
  #[cfg(all(
    feature = "bench-internals",
    target_os = "linux",
    any(
      target_arch = "x86",
      target_arch = "x86_64",
      target_arch = "aarch64",
      target_arch = "riscv64",
      target_arch = "powerpc64",
      target_arch = "s390x",
      target_arch = "loongarch64",
    )
  ))]
  fn read_clock_syscall(clock_id: i32) -> u64 {
    let mut ts = Timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: this is the raw Linux `clock_gettime` syscall with a writable timespec pointer.
    let rc = unsafe { syscall(SYS_CLOCK_GETTIME, clock_id, &mut ts) };
    if rc != 0 {
      panic!("clock_gettime syscall failed for clock id {clock_id}");
    }
    timespec_to_nanos(ts)
  }

  #[cfg(all(feature = "bench-internals", target_os = "linux", target_env = "gnu"))]
  type VdsoClockGettime = unsafe extern "C" fn(i32, *mut Timespec) -> i32;

  #[cfg(all(feature = "bench-internals", target_os = "linux", target_env = "gnu"))]
  unsafe extern "C" {
    fn dlopen(filename: *const c_char, flags: i32) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
  }

  #[inline(always)]
  #[cfg(all(feature = "bench-internals", target_os = "linux", target_env = "gnu"))]
  pub fn vdso_clock_monotonic() -> u64 {
    read_clock_vdso(CLOCK_MONOTONIC)
  }

  #[inline(always)]
  #[cfg(all(feature = "bench-internals", target_os = "linux", target_env = "gnu"))]
  pub fn vdso_clock_monotonic_raw() -> u64 {
    read_clock_vdso(CLOCK_MONOTONIC_RAW)
  }

  #[inline(always)]
  #[cfg(all(feature = "bench-internals", target_os = "linux", target_env = "gnu"))]
  fn read_clock_vdso(clock_id: i32) -> u64 {
    static VDSO_CLOCK_GETTIME: OnceLock<VdsoClockGettime> = OnceLock::new();
    let clock_gettime = VDSO_CLOCK_GETTIME.get_or_init(resolve_vdso_clock_gettime);
    let mut ts = Timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: the function pointer came from the process vDSO symbol table and uses the same
    // ABI as `clock_gettime`.
    let rc = unsafe { clock_gettime(clock_id, &mut ts) };
    if rc != 0 {
      panic!("vDSO clock_gettime failed for clock id {clock_id}");
    }
    timespec_to_nanos(ts)
  }

  #[cfg(all(feature = "bench-internals", target_os = "linux", target_env = "gnu"))]
  fn resolve_vdso_clock_gettime() -> VdsoClockGettime {
    for vdso in [b"linux-vdso.so.1\0".as_slice(), b"linux-gate.so.1\0".as_slice()] {
      // SAFETY: `vdso` is nul-terminated and names the kernel-provided vDSO image.
      let handle = unsafe { dlopen(vdso.as_ptr().cast::<c_char>(), 1) };
      if handle.is_null() {
        continue;
      }

      for symbol in [b"__vdso_clock_gettime\0".as_slice(), b"__kernel_clock_gettime\0".as_slice()] {
        // SAFETY: `handle` is a live `dlopen` handle and `symbol` is nul-terminated.
        let pointer = unsafe { dlsym(handle, symbol.as_ptr().cast::<c_char>()) };
        if !pointer.is_null() {
          // SAFETY: Linux exposes these symbols with the `clock_gettime` ABI.
          return unsafe { std::mem::transmute::<*mut c_void, VdsoClockGettime>(pointer) };
        }
      }
    }
    panic!("vDSO clock_gettime symbol not found");
  }
}

#[cfg(all(unix, not(target_os = "macos")))]
pub use monotonic::*;

#[cfg(not(unix))]
mod instant {
  use std::sync::OnceLock;
  use std::time::Instant;

  static START: OnceLock<Instant> = OnceLock::new();

  #[inline(always)]
  pub fn instant_elapsed() -> u64 {
    START.get_or_init(Instant::now).elapsed().as_nanos() as u64
  }

  #[cfg(all(feature = "bench-internals", target_os = "windows"))]
  unsafe extern "system" {
    fn QueryPerformanceCounter(counter: *mut i64) -> i32;
  }

  #[inline(always)]
  #[cfg(all(feature = "bench-internals", target_os = "windows"))]
  pub fn query_performance_counter() -> u64 {
    let mut counter = 0i64;
    // SAFETY: `counter` is a valid writable pointer for the duration of the call.
    let ok = unsafe { QueryPerformanceCounter(&mut counter) };
    if ok == 0 {
      panic!("QueryPerformanceCounter failed");
    }
    counter as u64
  }
}

#[cfg(not(unix))]
pub use instant::*;
