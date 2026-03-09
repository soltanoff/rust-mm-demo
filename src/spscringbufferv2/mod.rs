use crate::sync::{AtomicUsize, Ordering};

use std::cell::{Cell, UnsafeCell};
use std::ptr;

/// Single-Producer-Single-Consumer Ring Buffer.
/// (cache line optimized)
#[allow(dead_code)]
pub struct SPSCRingBufferV2<T> {
    capacity: usize,
    buffer: UnsafeCell<Box<[T]>>,
    head: AtomicUsize,
    tail: AtomicUsize,
    // Cache line optimisations
    cached_head: Cell<usize>,
    cached_tail: Cell<usize>,
}

unsafe impl<T: Send> Send for SPSCRingBufferV2<T> {}
unsafe impl<T: Send> Sync for SPSCRingBufferV2<T> {}

#[allow(dead_code)]
impl<T> SPSCRingBufferV2<T>
where
    T: Copy + Default,
{
    pub fn new(capacity: usize) -> Self {
        let buffer = vec![T::default(); capacity].into_boxed_slice();
        Self {
            capacity,
            buffer: UnsafeCell::new(buffer),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            // Cache line optimisations
            cached_head: Cell::new(0),
            cached_tail: Cell::new(0),
        }
    }

    pub fn try_produce(&self, value: T) -> bool {
        let current_tail = self.tail.load(Ordering::Relaxed);

        // Cache line optimizations
        // Обновлять cached_head только если буфер *кажется* заполненным на основе устаревшего кэша
        if self.next(current_tail) == self.cached_head.get() {
            self.cached_head.set(self.head.load(Ordering::Acquire));
        }

        let current_head = self.cached_head.get();

        if self.is_full(current_head, current_tail) {
            return false;
        }

        unsafe {
            let slot_ptr = self.slot_ptr(current_tail);
            ptr::write(slot_ptr, value);
        }

        self.tail.store(self.next(current_tail), Ordering::Release);

        true
    }

    pub fn try_consume(&self) -> Option<T> {
        let current_head = self.head.load(Ordering::Relaxed);

        // Cache line optimizations
        // Обновлять cached_head только если буфер *кажется* заполненным на основе устаревшего кэша
        if current_head == self.cached_tail.get() {
            self.cached_tail.set(self.tail.load(Ordering::Acquire));
        }

        let current_tail = self.cached_tail.get();

        if self.is_empty(current_head, current_tail) {
            return None;
        }

        let value = unsafe {
            let slot_ptr = self.slot_ptr(current_head);
            ptr::read(slot_ptr)
        };

        self.head.store(self.next(current_head), Ordering::Release);

        Some(value)
    }

    /// Возвращает сырой указатель `*mut T` на элемент буфера по заданному индексу.
    ///
    /// Обходит создание промежуточных ссылок (`&` / `&mut`) на весь слайс,
    /// чтобы не нарушать правила Stacked Borrows при одновременном доступе
    /// из потока-производителя и потока-потребителя к разным элементам буфера.
    ///
    /// `index` должен быть строго меньше `self.capacity`.
    fn slot_ptr(&self, index: usize) -> *mut T {
        unsafe {
            // self.buffer.get() -> *mut Box<[T]>
            // *self.buffer.get() -> Box<[T]> (place expression, без перемещения)
            // Сырой указатель на срез в куче, без промежуточной ссылки
            // &raw mut **self.buffer.get() -> *mut [T]
            let slice_ptr: *mut [T] = &raw mut **self.buffer.get();
            (slice_ptr as *mut T).add(index)
        }
    }

    fn next(&self, slot: usize) -> usize {
        (slot + 1) % self.capacity
    }

    fn is_full(&self, head: usize, tail: usize) -> bool {
        self.next(tail) == head
    }

    fn is_empty(&self, head: usize, tail: usize) -> bool {
        tail == head
    }
}

#[cfg(all(test, not(feature = "sanitizers")))]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_concurrent_reads_and_writes() {
        let ring_buffer: Arc<SPSCRingBufferV2<i32>> = Arc::new(SPSCRingBufferV2::new(42));
        let producer_buffer = Arc::clone(&ring_buffer);
        let consumer_buffer = Arc::clone(&ring_buffer);

        let values_count = 100500;

        let producer_handle = thread::spawn(move || {
            for i in 0..values_count {
                while !producer_buffer.try_produce(i) {
                    // Retry
                }
            }
        });

        let consumer_handle = thread::spawn(move || {
            for _ in 0..values_count {
                while consumer_buffer.try_consume().is_none() {
                    // Retry
                }
            }
        });

        producer_handle.join().unwrap();
        consumer_handle.join().unwrap();
    }
}

#[cfg(feature = "sanitizers")]
mod loom_tests {
    use super::*;
    use loom::sync::Arc;
    use loom::thread;

    #[test]
    fn loom_concurrent_reads_and_writes() {
        loom::model(|| {
            let ring_buffer: Arc<SPSCRingBufferV2<i32>> = Arc::new(SPSCRingBufferV2::new(2));
            let producer_buffer = Arc::clone(&ring_buffer);
            let consumer_buffer = Arc::clone(&ring_buffer);

            // АККУРАТНО! Большое значение порождает больше переборов исполнений согласно С11
            let values_count = 2;

            let producer_handle = thread::spawn(move || {
                for i in 0..values_count {
                    while !producer_buffer.try_produce(i) {
                        #[cfg(feature = "sanitizers")]
                        loom::thread::yield_now();
                    }
                }
            });

            let consumer_handle = thread::spawn(move || {
                for _ in 0..values_count {
                    while consumer_buffer.try_consume().is_none() {
                        #[cfg(feature = "sanitizers")]
                        loom::thread::yield_now();
                    }
                }
            });

            producer_handle.join().unwrap();
            consumer_handle.join().unwrap();
        });
    }
}
