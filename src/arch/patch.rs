use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering, compiler_fence};
#[cfg(all(windows, target_arch = "x86_64"))]
use std::sync::atomic::{AtomicPtr, AtomicU8, AtomicUsize};

static PATCH_LOCKED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PatchStats {
  pub(crate) registered: usize,
  pub(crate) patched: usize,
  pub(crate) already_patched: usize,
  pub(crate) failed: usize,
}

pub(crate) struct PatchLock;

impl PatchLock {
  pub(crate) fn acquire() -> Self {
    while PATCH_LOCKED
      .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
      .is_err()
    {
      std::hint::spin_loop();
    }
    Self
  }
}

impl Drop for PatchLock {
  fn drop(&mut self) {
    PATCH_LOCKED.store(false, Ordering::Release);
  }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod platform {
  use core::ffi::c_void;
  use std::ffi::c_int;

  const PROT_READ: c_int = 0x1;
  const PROT_WRITE: c_int = 0x2;
  const PROT_EXEC: c_int = 0x4;

  unsafe extern "C" {
    fn getpagesize() -> c_int;
    fn mprotect(addr: *mut c_void, len: usize, prot: c_int) -> c_int;
  }

  pub(crate) struct PageWriteGuard {
    start: *mut c_void,
    len: usize,
    active: bool,
  }

  impl PageWriteGuard {
    pub(crate) fn enable(ptr: *mut u8, len: usize) -> Result<Self, ()> {
      let (start, len) = page_span(ptr as usize, len)?;
      let mut guard = Self { start: start as *mut c_void, len, active: false };
      guard.mprotect(PROT_READ | PROT_WRITE | PROT_EXEC)?;
      guard.active = true;
      Ok(guard)
    }

    pub(crate) fn restore(mut self) -> Result<(), ()> {
      self.mprotect(PROT_READ | PROT_EXEC)?;
      self.active = false;
      Ok(())
    }

    fn mprotect(&self, protection: c_int) -> Result<(), ()> {
      // SAFETY: `start` and `len` are page-aligned values returned by `page_span`, and
      // `protection` is a valid platform page-protection bitset.
      if unsafe { mprotect(self.start, self.len, protection) } == 0 { Ok(()) } else { Err(()) }
    }
  }

  impl Drop for PageWriteGuard {
    fn drop(&mut self) {
      if self.active {
        let _ = self.mprotect(PROT_READ | PROT_EXEC);
      }
    }
  }

  fn page_span(addr: usize, len: usize) -> Result<(usize, usize), ()> {
    let page_size = page_size()?;
    let page_mask = !(page_size - 1);
    let start = addr & page_mask;
    let end = addr.checked_add(len).ok_or(())?;
    let end = end.checked_add(page_size - 1).ok_or(())? & page_mask;
    let len = end.checked_sub(start).ok_or(())?;
    Ok((start, len))
  }

  fn page_size() -> Result<usize, ()> {
    // SAFETY: `getpagesize` has no Rust-side safety preconditions.
    let value = unsafe { getpagesize() };
    if value <= 0 {
      return Err(());
    }
    usize::try_from(value).map_err(|_| ())
  }

  #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
  pub(crate) fn flush_instruction_cache(_ptr: *mut u8, _len: usize) {
    super::compiler_fence(super::Ordering::SeqCst);
  }

  #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
  pub(crate) fn flush_instruction_cache(ptr: *mut u8, len: usize) {
    unsafe extern "C" {
      fn __clear_cache(start: *mut c_void, end: *mut c_void);
    }

    // SAFETY: The range is the writable/executable code range that was just modified.
    unsafe {
      __clear_cache(ptr.cast::<c_void>(), ptr.add(len).cast::<c_void>());
    }
  }
}

#[cfg(target_os = "macos")]
mod platform {
  #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
  use core::ffi::c_void;
  use std::ffi::c_int;

  const KERN_SUCCESS: c_int = 0;
  const VM_PROT_READ: c_int = 0x1;
  const VM_PROT_WRITE: c_int = 0x2;
  const VM_PROT_EXECUTE: c_int = 0x4;
  const VM_PROT_COPY: c_int = 0x10;

  unsafe extern "C" {
    static mach_task_self_: u32;

    fn getpagesize() -> c_int;
    fn mach_vm_protect(
      target_task: u32,
      address: u64,
      size: u64,
      set_maximum: c_int,
      new_protection: c_int,
    ) -> c_int;
  }

  pub(crate) struct PageWriteGuard {
    start: usize,
    len: usize,
    active: bool,
  }

  impl PageWriteGuard {
    pub(crate) fn enable(ptr: *mut u8, len: usize) -> Result<Self, ()> {
      let (start, len) = page_span(ptr as usize, len)?;
      let mut guard = Self { start, len, active: false };
      guard.protect(VM_PROT_READ | VM_PROT_WRITE | VM_PROT_EXECUTE | VM_PROT_COPY)?;
      guard.active = true;
      Ok(guard)
    }

    pub(crate) fn restore(mut self) -> Result<(), ()> {
      self.protect(VM_PROT_READ | VM_PROT_EXECUTE)?;
      self.active = false;
      Ok(())
    }

    fn protect(&self, protection: c_int) -> Result<(), ()> {
      let address = u64::try_from(self.start).map_err(|_| ())?;
      let size = u64::try_from(self.len).map_err(|_| ())?;

      // SAFETY: `start` and `len` are page-aligned values returned by `page_span`.
      // `mach_task_self_` names the current process task port, and `VM_PROT_COPY` is the
      // Darwin-supported way to add write permission to a private executable mapping.
      let result = unsafe { mach_vm_protect(mach_task_self_, address, size, 0, protection) };
      if result == KERN_SUCCESS { Ok(()) } else { Err(()) }
    }
  }

  impl Drop for PageWriteGuard {
    fn drop(&mut self) {
      if self.active {
        let _ = self.protect(VM_PROT_READ | VM_PROT_EXECUTE);
      }
    }
  }

  fn page_span(addr: usize, len: usize) -> Result<(usize, usize), ()> {
    let page_size = page_size()?;
    let page_mask = !(page_size - 1);
    let start = addr & page_mask;
    let end = addr.checked_add(len).ok_or(())?;
    let end = end.checked_add(page_size - 1).ok_or(())? & page_mask;
    let len = end.checked_sub(start).ok_or(())?;
    Ok((start, len))
  }

  fn page_size() -> Result<usize, ()> {
    // SAFETY: `getpagesize` has no Rust-side safety preconditions.
    let value = unsafe { getpagesize() };
    if value <= 0 {
      return Err(());
    }
    usize::try_from(value).map_err(|_| ())
  }

  #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
  pub(crate) fn flush_instruction_cache(_ptr: *mut u8, _len: usize) {
    super::compiler_fence(super::Ordering::SeqCst);
  }

  #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
  pub(crate) fn flush_instruction_cache(ptr: *mut u8, len: usize) {
    unsafe extern "C" {
      fn sys_icache_invalidate(start: *mut c_void, len: usize);
    }

    // SAFETY: The range is the writable/executable code range that was just modified.
    unsafe {
      sys_icache_invalidate(ptr.cast::<c_void>(), len);
    }
  }
}

#[cfg(windows)]
mod platform {
  use core::ffi::c_void;
  use std::ptr::null_mut;

  pub(super) const EXCEPTION_BREAKPOINT: u32 = 0x8000_0003;
  pub(super) const EXCEPTION_CONTINUE_EXECUTION: i32 = -1;
  pub(super) const EXCEPTION_CONTINUE_SEARCH: i32 = 0;

  const PAGE_EXECUTE_READWRITE: u32 = 0x40;

  unsafe extern "system" {
    pub(super) fn AddVectoredExceptionHandler(
      first: u32,
      handler: VectoredExceptionHandler,
    ) -> *mut c_void;
    fn GetCurrentProcess() -> *mut c_void;
    fn FlushInstructionCache(process: *mut c_void, base: *const c_void, size: usize) -> i32;
    fn VirtualProtect(
      address: *mut c_void,
      size: usize,
      new_protect: u32,
      old_protect: *mut u32,
    ) -> i32;
  }

  pub(super) type VectoredExceptionHandler =
    Option<unsafe extern "system" fn(*mut ExceptionPointers) -> i32>;

  #[repr(C)]
  pub(super) struct ExceptionPointers {
    pub(super) exception_record: *mut ExceptionRecord,
    pub(super) context_record: *mut Context,
  }

  #[repr(C)]
  pub(super) struct ExceptionRecord {
    pub(super) exception_code: u32,
    pub(super) exception_flags: u32,
    pub(super) exception_record: *mut ExceptionRecord,
    pub(super) exception_address: *mut c_void,
    pub(super) number_parameters: u32,
    pub(super) exception_information: [usize; 15],
  }

  #[repr(C, align(16))]
  pub(super) struct Context {
    pub(super) p1_home: u64,
    pub(super) p2_home: u64,
    pub(super) p3_home: u64,
    pub(super) p4_home: u64,
    pub(super) p5_home: u64,
    pub(super) p6_home: u64,
    pub(super) context_flags: u32,
    pub(super) mx_csr: u32,
    pub(super) seg_cs: u16,
    pub(super) seg_ds: u16,
    pub(super) seg_es: u16,
    pub(super) seg_fs: u16,
    pub(super) seg_gs: u16,
    pub(super) seg_ss: u16,
    pub(super) eflags: u32,
    pub(super) dr0: u64,
    pub(super) dr1: u64,
    pub(super) dr2: u64,
    pub(super) dr3: u64,
    pub(super) dr6: u64,
    pub(super) dr7: u64,
    pub(super) rax: u64,
    pub(super) rcx: u64,
    pub(super) rdx: u64,
    pub(super) rbx: u64,
    pub(super) rsp: u64,
    pub(super) rbp: u64,
    pub(super) rsi: u64,
    pub(super) rdi: u64,
    pub(super) r8: u64,
    pub(super) r9: u64,
    pub(super) r10: u64,
    pub(super) r11: u64,
    pub(super) r12: u64,
    pub(super) r13: u64,
    pub(super) r14: u64,
    pub(super) r15: u64,
    pub(super) rip: u64,
  }

  pub(crate) struct PageWriteGuard {
    ptr: *mut c_void,
    len: usize,
    old_protect: u32,
    active: bool,
  }

  impl PageWriteGuard {
    pub(crate) fn enable(ptr: *mut u8, len: usize) -> Result<Self, ()> {
      let mut old_protect = 0;
      // SAFETY: `ptr..ptr+len` is a code range owned by this crate's emitted callsite.
      let ok = unsafe {
        VirtualProtect(ptr.cast::<c_void>(), len, PAGE_EXECUTE_READWRITE, &mut old_protect)
      };
      if ok == 0 {
        return Err(());
      }
      Ok(Self { ptr: ptr.cast::<c_void>(), len, old_protect, active: true })
    }

    pub(crate) fn restore(mut self) -> Result<(), ()> {
      let mut ignored = 0;
      // SAFETY: Restores the protection previously returned by `VirtualProtect`.
      let ok = unsafe { VirtualProtect(self.ptr, self.len, self.old_protect, &mut ignored) };
      if ok == 0 {
        return Err(());
      }
      self.active = false;
      Ok(())
    }
  }

  impl Drop for PageWriteGuard {
    fn drop(&mut self) {
      if self.active {
        let mut ignored = 0;
        // SAFETY: Best-effort restoration for the code range made writable by this guard.
        let _ = unsafe { VirtualProtect(self.ptr, self.len, self.old_protect, &mut ignored) };
      }
    }
  }

  pub(crate) fn flush_instruction_cache(ptr: *mut u8, len: usize) {
    // SAFETY: Flushes the current process instruction cache for the modified code range.
    unsafe {
      let process = GetCurrentProcess();
      let process = if process.is_null() { null_mut() } else { process };
      let _ = FlushInstructionCache(process, ptr.cast::<c_void>(), len);
    }
  }
}

pub(crate) use platform::{PageWriteGuard, flush_instruction_cache};

#[cfg(all(windows, target_arch = "x86_64"))]
const INT3: u8 = 0xCC;
#[cfg(all(windows, target_arch = "x86_64"))]
const VEH_UNINSTALLED: u8 = 0;
#[cfg(all(windows, target_arch = "x86_64"))]
const VEH_INSTALLING: u8 = 1;
#[cfg(all(windows, target_arch = "x86_64"))]
const VEH_INSTALLED: u8 = 2;
#[cfg(all(windows, target_arch = "x86_64"))]
const VEH_FAILED: u8 = 3;

#[cfg(all(windows, target_arch = "x86_64"))]
static VEH_STATE: AtomicU8 = AtomicU8::new(VEH_UNINSTALLED);
#[cfg(all(windows, target_arch = "x86_64"))]
static PATCHPOINTS: AtomicPtr<BreakpointPatchpoint> = AtomicPtr::new(core::ptr::null_mut());
#[cfg(all(windows, target_arch = "x86_64"))]
static ACTIVE_PATCHPOINT: AtomicUsize = AtomicUsize::new(0);

#[cfg(all(windows, target_arch = "x86_64"))]
struct BreakpointPatchpoint {
  address: usize,
  next: *mut BreakpointPatchpoint,
}

#[allow(clippy::manual_is_multiple_of)]
#[allow(dead_code)]
pub(crate) fn patch_bytes_with_u64_commit(ptr: *mut u8, bytes: &[u8], commit_len: usize) -> bool {
  if commit_len != 8 || bytes.len() < commit_len || (ptr as usize) % commit_len != 0 {
    return false;
  }

  let _lock = PatchLock::acquire();
  let Ok(guard) = PageWriteGuard::enable(ptr, bytes.len()) else {
    return false;
  };

  // SAFETY: The page guard made the crate-owned gate writable. Non-entry bytes are written
  // first; the aligned atomic store commits the first eight bytes in one step.
  unsafe {
    for (offset, byte) in bytes.iter().enumerate().skip(commit_len) {
      ptr.add(offset).write(*byte);
    }
    compiler_fence(Ordering::SeqCst);
    AtomicU64::from_ptr(ptr.cast::<u64>())
      .store(u64::from_ne_bytes(bytes[..commit_len].try_into().unwrap()), Ordering::SeqCst);
  }

  flush_instruction_cache(ptr, bytes.len());
  let _ = guard.restore();
  true
}

#[cfg(all(windows, target_arch = "x86_64"))]
#[allow(dead_code)]
pub(crate) fn patch_bytes_with_breakpoint_commit(ptr: *mut u8, bytes: &[u8]) -> bool {
  if bytes.is_empty() || !ensure_breakpoint_handler() {
    return false;
  }

  let _lock = PatchLock::acquire();
  register_breakpoint_patchpoint(ptr as usize);
  ACTIVE_PATCHPOINT.store(ptr as usize, Ordering::Release);

  let Ok(guard) = PageWriteGuard::enable(ptr, bytes.len()) else {
    ACTIVE_PATCHPOINT.store(0, Ordering::Release);
    return false;
  };

  // SAFETY: The page guard made this crate-owned gate writable. The first byte is changed to
  // `INT3`, which routes racing executions to the vectored handler while the tail is rewritten.
  unsafe {
    AtomicU8::from_ptr(ptr).store(INT3, Ordering::SeqCst);
  }
  flush_instruction_cache(ptr, 1);

  // SAFETY: Threads can no longer execute these tail bytes without first trapping on byte zero.
  unsafe {
    for (offset, byte) in bytes.iter().enumerate().skip(1) {
      ptr.add(offset).write(*byte);
    }
    compiler_fence(Ordering::SeqCst);
    AtomicU8::from_ptr(ptr).store(bytes[0], Ordering::SeqCst);
  }

  flush_instruction_cache(ptr, bytes.len());
  ACTIVE_PATCHPOINT.store(0, Ordering::Release);
  let _ = guard.restore();
  true
}

#[cfg(all(windows, target_arch = "x86_64"))]
fn ensure_breakpoint_handler() -> bool {
  loop {
    match VEH_STATE.load(Ordering::Acquire) {
      VEH_INSTALLED => return true,
      VEH_FAILED => return false,
      VEH_UNINSTALLED => {
        if VEH_STATE
          .compare_exchange(VEH_UNINSTALLED, VEH_INSTALLING, Ordering::AcqRel, Ordering::Acquire)
          .is_ok()
        {
          // SAFETY: Installs a process-wide handler that claims only registered tach patchpoints.
          let handle =
            unsafe { platform::AddVectoredExceptionHandler(1, Some(breakpoint_handler)) };
          let state = if handle.is_null() { VEH_FAILED } else { VEH_INSTALLED };
          VEH_STATE.store(state, Ordering::Release);
          return state == VEH_INSTALLED;
        }
      }
      VEH_INSTALLING => std::hint::spin_loop(),
      _ => return false,
    }
  }
}

#[cfg(all(windows, target_arch = "x86_64"))]
fn register_breakpoint_patchpoint(address: usize) {
  if registered_patchpoint(address, address).is_some() {
    return;
  }

  let node = Box::into_raw(Box::new(BreakpointPatchpoint { address, next: core::ptr::null_mut() }));

  loop {
    let head = PATCHPOINTS.load(Ordering::Acquire);
    // SAFETY: `node` is exclusively owned until it is published by the successful CAS.
    unsafe {
      (*node).next = head;
    }
    if PATCHPOINTS
      .compare_exchange(head, node, Ordering::AcqRel, Ordering::Acquire)
      .is_ok()
    {
      break;
    }
  }
}

#[cfg(all(windows, target_arch = "x86_64"))]
unsafe extern "system" fn breakpoint_handler(info: *mut platform::ExceptionPointers) -> i32 {
  if info.is_null() {
    return platform::EXCEPTION_CONTINUE_SEARCH;
  }

  // SAFETY: Windows calls the handler with a valid `EXCEPTION_POINTERS` for the current fault.
  let pointers = unsafe { &mut *info };
  if pointers.exception_record.is_null() || pointers.context_record.is_null() {
    return platform::EXCEPTION_CONTINUE_SEARCH;
  }

  // SAFETY: Both pointers were checked above and are owned by the OS during handler execution.
  let record = unsafe { &*pointers.exception_record };
  if record.exception_code != platform::EXCEPTION_BREAKPOINT {
    return platform::EXCEPTION_CONTINUE_SEARCH;
  }

  // SAFETY: The context pointer was checked above and is writable for continue-execution edits.
  let context = unsafe { &mut *pointers.context_record };
  let exception_address = record.exception_address as usize;
  let rip = usize::try_from(context.rip).unwrap_or(0);
  let Some(patchpoint) = registered_patchpoint(exception_address, rip) else {
    return platform::EXCEPTION_CONTINUE_SEARCH;
  };

  wait_for_breakpoint_patch(patchpoint);
  context.rip = patchpoint as u64;
  platform::EXCEPTION_CONTINUE_EXECUTION
}

#[cfg(all(windows, target_arch = "x86_64"))]
fn registered_patchpoint(exception_address: usize, rip: usize) -> Option<usize> {
  let mut node = PATCHPOINTS.load(Ordering::Acquire);
  while !node.is_null() {
    // SAFETY: Patchpoint nodes are leaked after publication, so published links remain valid.
    let entry = unsafe { &*node };
    if exception_address == entry.address
      || exception_address == entry.address + 1
      || rip == entry.address
      || rip == entry.address + 1
    {
      return Some(entry.address);
    }
    node = entry.next;
  }
  None
}

#[cfg(all(windows, target_arch = "x86_64"))]
fn wait_for_breakpoint_patch(patchpoint: usize) {
  loop {
    if ACTIVE_PATCHPOINT.load(Ordering::Acquire) != patchpoint {
      return;
    }

    // SAFETY: `patchpoint` is a registered executable address that remains readable.
    let first = unsafe { (patchpoint as *const u8).read_volatile() };
    if first != INT3 {
      return;
    }

    std::hint::spin_loop();
  }
}

#[allow(clippy::manual_is_multiple_of)]
#[allow(dead_code)]
pub(crate) fn patch_u32(ptr: *mut u8, word: u32) -> bool {
  if (ptr as usize) % 4 != 0 {
    return false;
  }

  let _lock = PatchLock::acquire();
  let Ok(guard) = PageWriteGuard::enable(ptr, 4) else {
    return false;
  };

  // SAFETY: The page guard made the aligned crate-owned gate writable.
  unsafe {
    AtomicU32::from_ptr(ptr.cast::<u32>()).store(word, Ordering::SeqCst);
  }

  flush_instruction_cache(ptr, 4);
  let _ = guard.restore();
  true
}

#[allow(dead_code)]
pub(crate) fn branch_i32(from: usize, to: usize, instruction_len: usize) -> Option<i32> {
  let from_next = from.checked_add(instruction_len)?;
  let diff = isize::try_from(to).ok()?.checked_sub(isize::try_from(from_next).ok()?)?;
  i32::try_from(diff).ok()
}

#[allow(dead_code)]
pub(crate) fn branch_words(from: usize, to: usize, shift: u32, bits: u32) -> Option<i32> {
  let diff = isize::try_from(to).ok()?.checked_sub(isize::try_from(from).ok()?)?;
  let unit = 1isize.checked_shl(shift)?;
  if diff % unit != 0 {
    return None;
  }
  let value = diff / unit;
  let min = -(1isize << (bits - 1));
  let max = (1isize << (bits - 1)) - 1;
  if value < min || value > max {
    return None;
  }
  i32::try_from(value).ok()
}
