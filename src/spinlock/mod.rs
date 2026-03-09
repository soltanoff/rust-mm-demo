use crate::sync::{AtomicBool, Ordering, pause};

/// Test-And-TAS SpinLock
/// (TAS -> Test-And-Set)
#[allow(dead_code)]
pub struct SpinLock {
    locked: AtomicBool,
}

#[allow(dead_code)]
pub struct SpinLockGuard<'a> {
    lock: &'a SpinLock,
}

#[allow(dead_code)]
impl SpinLock {
    pub fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
        }
    }

    pub fn lock(&self) {
        // RWM операция
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // Optimization: Read-only операция
            while self.locked.load(Ordering::Relaxed) {
                pause();
            }
        }
    }

    pub fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }
}

#[allow(dead_code)]
impl<'a> SpinLockGuard<'a> {
    pub fn new(lock: &'a SpinLock) -> Self {
        lock.lock();
        Self { lock }
    }
}

impl Drop for SpinLock {
    fn drop(&mut self) {
        self.unlock();
    }
}

impl<'a> Drop for SpinLockGuard<'a> {
    fn drop(&mut self) {
        self.lock.unlock();
    }
}

#[cfg(all(test, not(feature = "sanitizers")))]
mod tests {
    use super::*;
    use std::cell::UnsafeCell;
    use std::sync::Arc;

    struct SharedData<T> {
        lock: SpinLock,
        value: UnsafeCell<T>,
    }

    unsafe impl<T> Sync for SharedData<T> {}

    #[test]
    fn test_multiple_sequential_locks() {
        let lock = SpinLock::new();

        for _ in 0..100 {
            lock.lock();
            lock.unlock();
        }
    }

    #[test]
    fn test_concurrent_logic() {
        let v1 = Arc::new(SpinLock::new());
        let v2 = v1.clone();
        let t1 = std::thread::spawn(move || {
            v2.lock();
            v2.unlock();
        });

        v1.lock();
        v1.unlock();

        t1.join().unwrap();
    }

    #[test]
    fn test_two_threads_no_data_race() {
        let shared = Arc::new(SharedData {
            lock: SpinLock::new(),
            value: UnsafeCell::new(0u32),
        });

        let shared2 = Arc::clone(&shared);
        let t1 = std::thread::spawn(move || {
            shared2.lock.lock();
            unsafe { *shared2.value.get() += 1 };
            shared2.lock.unlock();
        });

        shared.lock.lock();
        unsafe { *shared.value.get() += 1 };
        shared.lock.unlock();

        t1.join().unwrap();

        assert_eq!(unsafe { *shared.value.get() }, 2);
    }

    #[test]
    fn test_mutual_exclusion() {
        let shared = Arc::new(SharedData {
            lock: SpinLock::new(),
            value: UnsafeCell::new(String::new()),
        });

        let shared2 = Arc::clone(&shared);
        let t1 = std::thread::spawn(move || {
            shared2.lock.lock();
            unsafe { (*shared2.value.get()).push('A') };
            shared2.lock.unlock();
        });

        shared.lock.lock();
        unsafe { (*shared.value.get()).push('B') };
        shared.lock.unlock();

        t1.join().unwrap();

        let val = unsafe { &*shared.value.get() };
        assert!(val == "AB" || val == "BA", "unexpected value: {}", val);
    }

    #[test]
    fn test_concurrent_increments() {
        let shared = Arc::new(SharedData {
            lock: SpinLock::new(),
            value: UnsafeCell::new(0u64),
        });

        let num_threads = 3;
        let increments_per_thread = 16750;

        let handles: Vec<_> = (0..num_threads)
            .map(|_| {
                let shared = Arc::clone(&shared);
                std::thread::spawn(move || {
                    for _ in 0..increments_per_thread {
                        shared.lock.lock();
                        unsafe { *shared.value.get() += 1 };
                        shared.lock.unlock();
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(
            unsafe { *shared.value.get() },
            num_threads * increments_per_thread
        );
    }
}

#[cfg(feature = "sanitizers")]
mod loom_tests {
    use super::*;
    use loom::sync::Arc;
    use loom::thread;
    use std::cell::UnsafeCell;

    struct SharedData<T> {
        lock: SpinLock,
        value: UnsafeCell<T>,
    }

    unsafe impl<T> Sync for SharedData<T> {}

    #[test]
    fn loom_multiple_sequential_locks() {
        loom::model(|| {
            let lock = SpinLock::new();

            for _ in 0..100 {
                lock.lock();
                lock.unlock();
            }
        });
    }

    #[test]
    fn loom_concurrent_logic() {
        loom::model(|| {
            let v1 = loom::sync::Arc::new(SpinLock::new());
            let v2 = v1.clone();
            let t1 = thread::spawn(move || {
                v2.lock();
                v2.unlock();
            });

            v1.lock();
            v1.unlock();

            t1.join().unwrap();
        });
    }

    #[test]
    fn loom_two_threads_no_data_race() {
        loom::model(|| {
            let shared = Arc::new(SharedData {
                lock: SpinLock::new(),
                value: UnsafeCell::new(0u32),
            });

            let shared2 = Arc::clone(&shared);
            let t1 = thread::spawn(move || {
                shared2.lock.lock();
                unsafe { *shared2.value.get() += 1 };
                shared2.lock.unlock();
            });

            shared.lock.lock();
            unsafe { *shared.value.get() += 1 };
            shared.lock.unlock();

            t1.join().unwrap();

            assert_eq!(unsafe { *shared.value.get() }, 2);
        });
    }

    #[test]
    fn loom_mutual_exclusion() {
        loom::model(|| {
            let shared = Arc::new(SharedData {
                lock: SpinLock::new(),
                value: UnsafeCell::new(String::new()),
            });

            let shared2 = Arc::clone(&shared);
            let t1 = thread::spawn(move || {
                shared2.lock.lock();
                unsafe { (*shared2.value.get()).push('A') };
                shared2.lock.unlock();
            });

            shared.lock.lock();
            unsafe { (*shared.value.get()).push('B') };
            shared.lock.unlock();

            t1.join().unwrap();

            let val = unsafe { &*shared.value.get() };
            assert!(val == "AB" || val == "BA", "unexpected value: {}", val);
        });
    }

    #[test]
    fn loom_concurrent_increments() {
        loom::model(|| {
            let shared = Arc::new(SharedData {
                lock: SpinLock::new(),
                value: UnsafeCell::new(0u64),
            });

            let num_threads = 2;
            // АККУРАТНО! Большое значение порождает больше переборов исполнений согласно С11
            let appends_per_thread = 2;

            let handles: Vec<_> = (0..num_threads)
                .map(|_| {
                    let shared = Arc::clone(&shared);
                    thread::spawn(move || {
                        for _ in 0..appends_per_thread {
                            shared.lock.lock();
                            unsafe { *shared.value.get() += 1 };
                            shared.lock.unlock();
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }

            assert_eq!(
                unsafe { *shared.value.get() },
                num_threads * appends_per_thread
            );
        });
    }
}
