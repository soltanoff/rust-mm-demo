use crate::sync::{AtomicUsize, Ordering, fence};

use std::ops::Deref;

struct SharedInner<T> {
    ref_count: AtomicUsize,
    data: T,
}

/// Оптимизированная реализация shared pointer с подсчётом ссылок.
///
/// В отличие от V1 (AcqRel везде), здесь ordering'и ослаблены до минимально необходимых:
/// - `clone()`: `fetch_add(1, Relaxed)` безопасно, т.к. клонирующий уже владеет ссылкой
/// - `drop()`: `fetch_sub(1, Release)` публикует записи потока
/// - Деструктор: `fence(Acquire)` только при count == 0 синхронизируется с release sequence
///
/// Это паттерн из `std::sync::Arc`.
#[allow(dead_code)]
pub struct SharedPtr<T> {
    inner: *mut SharedInner<T>,
}

unsafe impl<T: Send + Sync> Send for SharedPtr<T> {}
unsafe impl<T: Send + Sync> Sync for SharedPtr<T> {}

#[allow(dead_code)]
impl<T> SharedPtr<T> {
    pub fn new(data: T) -> Self {
        let inner = Box::into_raw(Box::new(SharedInner {
            ref_count: AtomicUsize::new(1),
            data,
        }));

        Self { inner }
    }
}

impl<T> Deref for SharedPtr<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &(*self.inner).data }
    }
}

impl<T> Clone for SharedPtr<T> {
    fn clone(&self) -> Self {
        // Relaxed безопасен здесь:
        // 1. Клонирующий поток уже владеет SharedPtr → ref_count >= 1 → объект жив
        // 2. Мы не читаем данные через результат fetch_add
        // 3. Нам не нужна синхронизация: мы только увеличиваем счётчик
        //
        // Единственный инвариант: ref_count не должен переполниться.
        // На практике это невозможно (usize::MAX ссылок).
        unsafe { &*self.inner }
            .ref_count
            .fetch_add(1, Ordering::Relaxed);

        Self { inner: self.inner }
    }
}

impl<T> Drop for SharedPtr<T> {
    fn drop(&mut self) {
        // Release на fetch_sub:
        // Публикует все записи этого потока (включая модификации данных через &T/&mut T)
        // для потока, который последним уменьшит счётчик до 0.
        //
        // Мы НЕ используем Acquire здесь это горячий путь, и на ARM/слабых архитектурах
        // Release дешевле AcqRel.
        let prev = unsafe { &*self.inner }
            .ref_count
            .fetch_sub(1, Ordering::Release);

        // prev == 1 означает, что именно этот поток атомарно перевёл счётчик 1 -> 0.
        // DCL не нужен: fetch_sub — неделимая RMW-операция, только один поток
        // может наблюдать prev == 1.
        // fetch_add (clone) в этот момент невозможен: для clone нужен живой SharedPtr,
        // а единственный владелец мы, и мы его дропаем.
        // https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html
        if prev == 1 {
            // fence(Acquire) синхронизируется с Release-операциями из release sequence,
            // headed by первым Release store в ref_count.
            //
            // Это гарантирует, что деструктор видит ВСЕ записи всех потоков,
            // которые ранее дропали свои SharedPtr.
            //
            // Fence вместо AcqRel на fetch_sub оптимизация:
            // fence выполняется только на холодном пути (count == 0),
            // а не на каждом drop.
            fence(Ordering::Acquire);

            unsafe {
                drop(Box::from_raw(self.inner));
            }
        }
    }
}

#[cfg(all(test, not(feature = "sanitizers")))]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    struct Tracked<'a> {
        value: i32,
        drop_count: &'a AtomicUsize,
    }

    impl<'a> Drop for Tracked<'a> {
        fn drop(&mut self) {
            self.drop_count.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn test_basic_creation_and_deref() {
        let ptr = SharedPtr::new(42);

        assert_eq!(*ptr, 42);
    }

    #[test]
    fn test_clone_and_drop() {
        let drop_count = AtomicUsize::new(0);
        {
            let ptr = SharedPtr::new(Tracked {
                value: 10,
                drop_count: &drop_count,
            });

            assert_eq!(ptr.value, 10);

            let ptr2 = ptr.clone();

            assert_eq!(ptr2.value, 10);

            drop(ptr);

            assert_eq!(drop_count.load(Ordering::SeqCst), 0);
        }
        assert_eq!(drop_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_concurrent_clone_drop() {
        let drop_count = Arc::new(AtomicUsize::new(0));

        let ptr = SharedPtr::new(42);

        let mut handles = Vec::new();

        for _ in 0..10 {
            let cloned = ptr.clone();
            let dc = Arc::clone(&drop_count);
            handles.push(thread::spawn(move || {
                assert_eq!(*cloned, 42);
                drop(cloned);
                dc.fetch_add(1, Ordering::SeqCst);
            }));
        }

        drop(ptr);

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(drop_count.load(Ordering::SeqCst), 10);
    }

    /// Сценарий с диаграммы:
    /// T1 создаёт объект (Bar), передаёт SharedPtr в T2,
    /// T2 вызывает метод через deref (Foo), оба дропают.
    #[test]
    fn test_scenario_bar_foo() {
        struct Bar {
            data: i32,
        }

        impl Bar {
            fn foo(&self) -> i32 {
                self.data * 2
            }
        }

        let p = SharedPtr::new(Bar { data: 21 });
        let p_clone = p.clone();

        let t2 = thread::spawn(move || {
            let result = p_clone.foo();
            assert_eq!(result, 42);
        });

        drop(p);

        t2.join().unwrap();
    }
}

#[cfg(feature = "sanitizers")]
mod loom_tests {
    use super::*;
    use loom::sync::Arc;
    use loom::sync::atomic::{AtomicUsize, Ordering};
    use loom::thread;

    struct Tracked {
        value: i32,
        drop_count: Arc<AtomicUsize>,
    }

    impl Drop for Tracked {
        fn drop(&mut self) {
            self.drop_count.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn test_basic_creation_and_deref() {
        loom::model(|| {
            let ptr = SharedPtr::new(42);

            assert_eq!(*ptr, 42);
        });
    }

    #[test]
    fn test_clone_and_drop() {
        loom::model(|| {
            let drop_count = Arc::new(AtomicUsize::new(0));
            {
                let ptr = SharedPtr::new(Tracked {
                    value: 10,
                    drop_count: Arc::clone(&drop_count),
                });

                assert_eq!(ptr.value, 10);

                let ptr2 = ptr.clone();

                assert_eq!(ptr2.value, 10);

                drop(ptr);

                assert_eq!(drop_count.load(Ordering::SeqCst), 0);
            }
            assert_eq!(drop_count.load(Ordering::SeqCst), 1);
        });
    }

    #[test]
    fn test_concurrent_clone_drop() {
        loom::model(|| {
            // АККУРАТНО! Loom ограничивает количество потоков (по умолчанию 4),
            // а каждый дополнительный поток экспоненциально увеличивает число переборов.
            let drop_count = Arc::new(AtomicUsize::new(0));

            let ptr = SharedPtr::new(Tracked {
                value: 42,
                drop_count: Arc::clone(&drop_count),
            });

            let cloned1 = ptr.clone();
            let cloned2 = ptr.clone();

            let t1 = thread::spawn(move || {
                assert_eq!(cloned1.value, 42);
                drop(cloned1);
            });

            let t2 = thread::spawn(move || {
                assert_eq!(cloned2.value, 42);
                drop(cloned2);
            });

            drop(ptr);

            t1.join().unwrap();
            t2.join().unwrap();

            assert_eq!(drop_count.load(Ordering::SeqCst), 1);
        });
    }

    /// Сценарий с диаграммы:
    /// T1 создаёт объект (Bar), передаёт SharedPtr в T2,
    /// T2 вызывает метод через deref (Foo), оба дропают.
    #[test]
    fn test_scenario_bar_foo() {
        loom::model(|| {
            struct Bar {
                data: i32,
            }

            impl Bar {
                fn foo(&self) -> i32 {
                    self.data * 2
                }
            }

            let p = SharedPtr::new(Bar { data: 21 });
            let p_clone = p.clone();

            let t2 = thread::spawn(move || {
                let result = p_clone.foo();
                assert_eq!(result, 42);
            });

            drop(p);

            t2.join().unwrap();
        });
    }
}
