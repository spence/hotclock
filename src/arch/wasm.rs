//! Performance.now() backed counter for wasm32-unknown-unknown.
//!
//! Uses `globalThis.performance.now()` so the binding works in both the main
//! thread and in Web Workers. Returns nanoseconds; the corresponding
//! `read_frequency()` in `super` returns 1_000_000_000 so the Q32 conversion
//! is an identity transform.
//!
//! Resolution is browser-clamped (typically ~100 microseconds for Spectre
//! mitigation). Successive `Instant::now()` calls within that window return
//! identical values; this satisfies non-decreasing monotonicity but not
//! strict monotonicity.

use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
unsafe extern "C" {
  #[wasm_bindgen(js_namespace = ["globalThis", "performance"], js_name = now)]
  fn performance_now() -> f64;
}

#[inline(always)]
#[allow(clippy::inline_always)]
pub fn ticks() -> u64 {
  let ms = performance_now();
  (ms * 1_000_000.0) as u64
}
