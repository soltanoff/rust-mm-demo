#[cfg(feature = "sanitizers")]
use loom::hint;
#[cfg(not(feature = "sanitizers"))]
use std::hint;

#[cfg(feature = "sanitizers")]
pub use loom::sync::atomic::{AtomicBool, Ordering};
#[cfg(not(feature = "sanitizers"))]
pub use std::sync::atomic::{AtomicBool, Ordering};

pub fn pause() {
    #[cfg(feature = "sanitizers")]
    loom::thread::yield_now();

    hint::spin_loop();
}
