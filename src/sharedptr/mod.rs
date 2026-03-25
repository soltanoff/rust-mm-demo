use crate::sync::{AtomicUsize, Ordering};

use std::ops::Deref;

struct SharedInner<T> {
    ref_count: AtomicUsize,
    data: T,
}

/// Наивная реализация shared pointer с подсчётом ссылок.
///
/// Все RMW-операции над ref_count используют `AcqRel`:
/// - Acquire-часть гарантирует видимость предыдущих записей других потоков
/// - Release-часть публикует изменение счётчика для последующих наблюдателей
///
/// Это корректно, но не оптимально.
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
        // AcqRel на fetch_add:
        // - Release публикует инкремент счётчика
        // - Acquire видит предыдущие записи в ref_count (modification order)
        //
        // Все fetch_add/fetch_sub образуют release sequence в modification order ref_count.
        unsafe { &*self.inner }
            .ref_count
            .fetch_add(1, Ordering::AcqRel);

        Self { inner: self.inner }
    }
}

impl<T> Drop for SharedPtr<T> {
    fn drop(&mut self) {
        // AcqRel на fetch_sub:
        // - Release публикует все записи этого потока (включая модификации данных)
        // - Acquire (при prev == 1) гарантирует, что деструктор видит ВСЕ записи
        //   всех потоков, которые ранее дропали свои SharedPtr
        //
        // Release sequence: цепочка RMW-операций (fetch_add/fetch_sub) в modification order
        // ref_count сохраняет synchronizes-with через промежуточные потоки.
        // Последний поток (prev == 1) через Acquire-часть синхронизируется
        // со всей release sequence.
        let prev = unsafe { &*self.inner }
            .ref_count
            .fetch_sub(1, Ordering::AcqRel);

        // prev == 1 означает, что именно этот поток атомарно перевёл счётчик 1 -> 0.
        // DCL не нужен: fetch_sub — неделимая RMW-операция, только один поток
        // может наблюдать prev == 1.
        // fetch_add (clone) в этот момент невозможен: для clone нужен живой SharedPtr,
        // а единственный владелец мы, и мы его дропаем.
        // https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html
        if prev == 1 {
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

        // T1: создаёт Bar
        let p = SharedPtr::new(Bar { data: 21 });
        let p_clone = p.clone();

        // T2: получает SharedPtr, вызывает Foo
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

    #[derive(Debug)]
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

            // T1: создаёт Bar
            let p = SharedPtr::new(Bar { data: 21 });
            let p_clone = p.clone();

            // T2: получает SharedPtr, вызывает Foo
            let t2 = thread::spawn(move || {
                let result = p_clone.foo();
                assert_eq!(result, 42);
            });

            drop(p);

            t2.join().unwrap();
        });
    }
}
