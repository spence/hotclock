use hotclock::Instant;

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn concurrent_first_calls_complete_and_install_one_counter() {
  use std::sync::Arc;
  use std::sync::Barrier;
  use std::sync::atomic::{AtomicBool, Ordering};

  const THREADS: usize = 16;
  const CALLS: usize = 1000;

  let start = Arc::new(Barrier::new(THREADS));
  let failed = Arc::new(AtomicBool::new(false));
  let mut threads = Vec::with_capacity(THREADS);

  for _ in 0..THREADS {
    let start = Arc::clone(&start);
    let failed = Arc::clone(&failed);
    threads.push(std::thread::spawn(move || {
      start.wait();

      let mut previous = Instant::now();
      for _ in 0..CALLS {
        let current = Instant::now();
        if current < previous {
          failed.store(true, Ordering::Relaxed);
          break;
        }
        previous = current;
      }
    }));
  }

  for thread in threads {
    thread.join().expect("clock thread panicked");
  }

  assert!(!failed.load(Ordering::Relaxed));
  assert!(matches!(Instant::implementation(), "x86_64-rdtsc" | "unix-monotonic"));
}

#[test]
#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
fn x86_64_linux_patch_test_is_target_specific() {
  assert!(!Instant::implementation().is_empty());
}
