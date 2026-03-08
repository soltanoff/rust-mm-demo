use crate::spinlock::SpinLock;
use crate::sync::{AtomicPtr, Ordering};
use std::cell::UnsafeCell;

/// Lazy Thread-Safe Initializer.
/// (e.g. init only once)
#[allow(dead_code)]
pub struct Lazy<T> {
    mutex: SpinLock,
    cell: AtomicPtr<T>,
    init: UnsafeCell<Box<dyn Fn() -> T + Send + Sync>>,
}

unsafe impl<T: Send + Sync> Send for Lazy<T> {}
unsafe impl<T: Send + Sync> Sync for Lazy<T> {}

#[allow(dead_code)]
impl<T> Lazy<T> {
    pub fn new(init: impl Fn() -> T + Send + Sync + 'static) -> Self {
        Lazy {
            mutex: SpinLock::new(),
            cell: AtomicPtr::new(std::ptr::null_mut()),
            init: UnsafeCell::new(Box::new(init)),
        }
    }

    /// Double-checked locking pattern
    pub fn access(&self) -> &T {
        // Fast path
        let mut ptr = self.cell.load(Ordering::Acquire); // Здесь мог быть Consume :)

        // Slow path
        if ptr.is_null() {
            self.mutex.lock();

            // Double-check после захвата mutex
            ptr = self.cell.load(Ordering::Relaxed);
            if ptr.is_null() {
                unsafe {
                    ptr = Box::into_raw(Box::new((*self.init.get())()));
                    self.cell.store(ptr, Ordering::Release);
                }
            }

            self.mutex.unlock();
        }

        unsafe { &*ptr }
    }
}

impl<T> Drop for Lazy<T> {
    fn drop(&mut self) {
        self.mutex.unlock();
        let ptr = self.cell.load(Ordering::Relaxed);
        if !ptr.is_null() {
            unsafe {
                drop(Box::from_raw(ptr));
            }
        }
    }
}

#[cfg(all(test, not(feature = "sanitizers")))]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::thread;

    #[test]
    fn test_lazy_initialization() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = Arc::clone(&call_count);

        let lazy: Arc<Lazy<i32>> = Arc::new(Lazy::new(move || {
            call_count_clone.fetch_add(1, Ordering::SeqCst);
            42
        }));

        let num_threads = 4;

        let handles: Vec<_> = (0..num_threads)
            .map(|_| {
                let shared = Arc::clone(&lazy);
                thread::spawn(move || {
                    let value = shared.access();
                    assert_eq!(*value, 42);
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }
}

#[cfg(feature = "sanitizers")]
mod loom_tests {
    use super::*;
    use loom::sync::Arc;
    use loom::sync::atomic::AtomicUsize;
    use loom::thread;

    #[test]
    fn test_lazy_initialization() {
        loom::model(|| {
            let call_count = Arc::new(AtomicUsize::new(0));
            let call_count_clone = Arc::clone(&call_count);

            let lazy: Arc<Lazy<i32>> = Arc::new(Lazy::new(move || {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                42
            }));

            let num_threads = 2;

            let handles: Vec<_> = (0..num_threads)
                .map(|_| {
                    let shared = Arc::clone(&lazy);
                    thread::spawn(move || {
                        let value = shared.access();
                        assert_eq!(*value, 42);
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }

            assert_eq!(call_count.load(Ordering::SeqCst), 1);
        })
    }
}
