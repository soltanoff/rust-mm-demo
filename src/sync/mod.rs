#[cfg(feature = "sanitizers")]
use loom::hint;
#[cfg(feature = "sanitizers")]
#[allow(unused_imports)]
use loom::sync::atomic::{AtomicBool, Ordering};

#[cfg(not(feature = "sanitizers"))]
use std::hint;
#[cfg(not(feature = "sanitizers"))]
#[allow(unused_imports)]
use std::sync::atomic::{AtomicBool, Ordering};

pub fn pause() {
    #[cfg(feature = "sanitizers")]
    loom::thread::yield_now();

    hint::spin_loop();
}
