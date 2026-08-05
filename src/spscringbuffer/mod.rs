#![allow(dead_code)]

use crate::sync::{AtomicUsize, Ordering};

use std::cell::UnsafeCell;
use std::ptr;

/// Single-Producer-Single-Consumer Ring Buffer
pub struct SPSCRingBuffer<T> {
    capacity: usize,
    // Слоты Option<T>: None = пусто, Some = занято. Позволяет move-семантику для
    // произвольного T (не требуется Copy/Default).
    buffer: UnsafeCell<Box<[Option<T>]>>,
    head: AtomicUsize,
    tail: AtomicUsize,
}

unsafe impl<T: Send> Send for SPSCRingBuffer<T> {}
unsafe impl<T: Send> Sync for SPSCRingBuffer<T> {}

impl<T> SPSCRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        // Инициализируем None без Clone/Default (не vec![None; n], который требует Clone).
        let buffer = (0..capacity)
            .map(|_| None)
            .collect::<Vec<Option<T>>>()
            .into_boxed_slice();
        Self {
            capacity,
            buffer: UnsafeCell::new(buffer),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    /// Кладёт значение по move. При полном буфере возвращает его обратно (`Err`),
    /// чтобы не потерять для не-`Copy` типов.
    pub fn try_produce(&self, value: T) -> Result<(), T> {
        let current_head = self.head.load(Ordering::Acquire);
        let current_tail = self.tail.load(Ordering::Relaxed);

        if self.is_full(current_head, current_tail) {
            return Err(value);
        }

        unsafe {
            let slot_ptr = self.slot_ptr(current_tail);
            // Слот пуст (None) + пишем Some.
            // ptr::write не дропает старое, но None ничего не держит - утечки нет.
            ptr::write(slot_ptr, Some(value));
        }

        self.tail.store(self.next(current_tail), Ordering::Release);

        Ok(())
    }

    pub fn try_consume(&self) -> Option<T> {
        let current_head = self.head.load(Ordering::Relaxed);
        let current_tail = self.tail.load(Ordering::Acquire);

        if self.is_empty(current_head, current_tail) {
            return None;
        }

        let value = unsafe {
            let slot_ptr = self.slot_ptr(current_head);
            // Вынимаем значение по move и оставляем None: иначе при drop буфера
            // снятый элемент дропнулся бы повторно (double-free).
            let taken = ptr::read(slot_ptr); // move Option<T> наружу (Some)
            ptr::write(slot_ptr, None); // затираем None; вынесенные байты не дропаем
            taken
        };

        self.head.store(self.next(current_head), Ordering::Release);

        value
    }

    /// Возвращает сырой указатель `*mut Option<T>` на слот буфера по заданному индексу.
    ///
    /// Обходит создание промежуточных ссылок (`&` / `&mut`) на весь слайс,
    /// чтобы не нарушать правила Stacked Borrows при одновременном доступе
    /// из потока-производителя и потока-потребителя к разным элементам буфера.
    ///
    /// `index` должен быть строго меньше `self.capacity`.
    fn slot_ptr(&self, index: usize) -> *mut Option<T> {
        unsafe {
            // self.buffer.get() -> *mut Box<[Option<T>]>
            // *self.buffer.get() -> Box<[Option<T>]> (place expression, без перемещения)
            // Сырой указатель на срез в куче, без промежуточной ссылки
            // &raw mut **self.buffer.get() -> *mut [Option<T>]
            let slice_ptr: *mut [Option<T>] = &raw mut **self.buffer.get();
            (slice_ptr as *mut Option<T>).add(index)
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
        let ring_buffer: Arc<SPSCRingBuffer<i32>> = Arc::new(SPSCRingBuffer::new(42));
        let producer_buffer = Arc::clone(&ring_buffer);
        let consumer_buffer = Arc::clone(&ring_buffer);

        let values_count = 100500;

        let producer_handle = thread::spawn(move || {
            for i in 0..values_count {
                while producer_buffer.try_produce(i).is_err() {
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

    /// Move-семантика: payload не обязан быть `Copy`. Кладём/снимаем `String`, а не снятый
    /// элемент дропается при drop буфера ровно один раз (Miri ловит утечку/double-free).
    #[test]
    fn test_move_non_copy_and_drop() {
        let rb: SPSCRingBuffer<String> = SPSCRingBuffer::new(4);
        rb.try_produce("a".to_string()).unwrap();
        rb.try_produce("b".to_string()).unwrap();
        assert_eq!(rb.try_consume(), Some("a".to_string())); // вынос по move
        // "b" остаётся в буфере -> дропнется при drop(rb).
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
            let ring_buffer: Arc<SPSCRingBuffer<i32>> = Arc::new(SPSCRingBuffer::new(2));
            let producer_buffer = Arc::clone(&ring_buffer);
            let consumer_buffer = Arc::clone(&ring_buffer);

            // АККУРАТНО! Большое значение порождает больше переборов исполнений согласно С11
            let values_count = 2;

            let producer_handle = thread::spawn(move || {
                for i in 0..values_count {
                    while producer_buffer.try_produce(i).is_err() {
                        loom::thread::yield_now();
                    }
                }
            });

            let consumer_handle = thread::spawn(move || {
                for _ in 0..values_count {
                    while consumer_buffer.try_consume().is_none() {
                        loom::thread::yield_now();
                    }
                }
            });

            producer_handle.join().unwrap();
            consumer_handle.join().unwrap();
        });
    }
}
