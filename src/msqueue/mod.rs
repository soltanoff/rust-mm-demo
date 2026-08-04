#![allow(dead_code)]

use crate::sync::{AtomicUsize, Ordering, pause};

// "Нет узла" / конец списка. Очередь НЕ переиспользует узлы (bump-аллокация),
// поэтому индекс достаточно хранить как есть, без version-тега: раз узел никогда не
// возвращается в пул, ABA на head/tail/next невозможна и тегам нечего защищать.
const NULL_INDEX: usize = u32::MAX as usize;

struct Node {
    // Payload. Без реюза узла value пишется ровно один раз (enqueue) и читается ровно
    // один раз (dequeue) - конкурентной записи в один слот нет, значит нет и data race
    // на value. AtomicUsize держит доступ безопасным (без ручного небезопасного кода).
    value: AtomicUsize,
    // Индекс следующего узла; NULL_INDEX -> конец списка.
    next: AtomicUsize,
}

/// Lock-free MPMC-очередь Майкла-Скотта с механикой **helping**.
///
/// Узлы НЕ переиспользуются: enqueue берёт свежий индекс из пула bump-аллокатором, а
/// dequeue снятые узлы не освобождает. Это делает очередь провабельно корректной на
/// `acq_rel` без hazard pointers: раз узел никогда не возвращается в пул, ABA и
/// use-after-free невозможны, а version-теги на указателях не нужны.
///
/// Цена - ограниченная ёмкость на ВСЁ ВРЕМЯ ЖИЗНИ: снятые слоты не возвращаются в
/// оборот. Реальный реюз узлов потребовал бы безопасного reclamation (hazard pointers /
/// epoch), см. README.
pub struct MSQueue {
    pool: Box<[Node]>, // фиксированная ёмкость, без реаллокации; доступ по индексу
    bump: AtomicUsize, // следующий невыданный индекс пула (bump-аллокатор)
    head: AtomicUsize, // индекс dummy-узла
    tail: AtomicUsize, // индекс последнего (или предпоследнего) узла
}
// Все поля атомарны -> MSQueue авто-Send + Sync; явная реализация не требуется.

impl MSQueue {
    /// Создаёт очередь на не более чем `capacity` элементов ЗА ВСЁ ВРЕМЯ ЖИЗНИ (узлы
    /// не переиспользуются). Выделяет `capacity + 1` узлов: индекс 0 - стартовый dummy,
    /// индексы 1..=capacity выдаёт bump-аллокатор под элементы.
    pub fn with_capacity(capacity: usize) -> Self {
        assert!(capacity >= 1, "ёмкость должна быть >= 1");
        let total = capacity + 1; // +1 под dummy (индекс 0)

        let mut nodes = Vec::with_capacity(total);
        for _ in 0..total {
            nodes.push(Node {
                value: AtomicUsize::new(0),
                next: AtomicUsize::new(NULL_INDEX),
            });
        }

        Self {
            pool: nodes.into_boxed_slice(),
            bump: AtomicUsize::new(1), // индекс 0 занят стартовым dummy
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    #[inline]
    fn node(&self, index: usize) -> &Node {
        &self.pool[index]
    }

    /// Выдать свежий индекс из пула (bump). `None` = пул исчерпан на всё время жизни.
    fn alloc(&self) -> Option<usize> {
        let idx = self.bump.fetch_add(1, Ordering::Relaxed);
        if idx < self.pool.len() {
            Some(idx)
        } else {
            None
        }
    }

    /// Поставить элемент. Два шага: (1) пристегнуть узел к `tail.next`, (2) продвинуть
    /// `tail`. Между ними `tail` отстаёт - тут и работает helping.
    pub fn enqueue(&self, value: usize) -> Result<(), usize> {
        let new_idx = match self.alloc() {
            Some(i) => i,
            None => return Err(value), // пул исчерпан на всё время жизни
        };
        // Готовим узел. Публикация произойдёт Release-частью CAS над next ниже, поэтому
        // здесь достаточно Relaxed.
        self.node(new_idx).value.store(value, Ordering::Relaxed);
        self.node(new_idx).next.store(NULL_INDEX, Ordering::Relaxed);

        loop {
            let t = self.tail.load(Ordering::Acquire);
            let t_next = self.node(t).next.load(Ordering::Acquire);
            // Перепроверка: tail не сдвинулся между двумя загрузками.
            if t != self.tail.load(Ordering::Acquire) {
                continue;
            }

            if t_next == NULL_INDEX {
                // Пытаемся пристегнуть узел. Release публикует value/next нового узла.
                if self
                    .node(t)
                    .next
                    .compare_exchange(NULL_INDEX, new_idx, Ordering::Release, Ordering::Relaxed)
                    .is_ok()
                {
                    // best-effort: двигаем tail на новый узел (промах не критичен -
                    // помогут другие потоки).
                    let _ =
                        self.tail
                            .compare_exchange(t, new_idx, Ordering::AcqRel, Ordering::Relaxed);
                    return Ok(());
                }
            } else {
                // HELPING: кто-то пристегнул узел, но не сдвинул tail. Помогаем.
                let _ = self
                    .tail
                    .compare_exchange(t, t_next, Ordering::AcqRel, Ordering::Relaxed);
            }
            pause();
        }
    }

    /// Снять элемент.
    ///
    /// ВАЖНО (модель памяти): пустоту определяем ТОЛЬКО по `next` (есть ли у dummy
    /// преемник), а НЕ по сравнению `head`/`tail`. Под слабой моделью памяти `Acquire`-
    /// загрузка `tail` вправе вернуть УСТАРЕВШЕЕ значение из modification order, поэтому
    /// согласованного среза `head`/`tail` там нет: `head != tail` НЕ гарантирует
    /// непустоту. Единственный надёжный признак пустоты - `next == NULL`. Устаревшая
    /// загрузка `tail` может лишь заставить нас лишний раз "помочь" (безобидно) или
    /// транзитно обогнать `tail` - и это тоже безопасно, потому что узлы НЕ
    /// освобождаются: `tail` всегда указывает на живой узел, и helping его догонит.
    pub fn dequeue(&self) -> Option<usize> {
        loop {
            let h = self.head.load(Ordering::Acquire);
            let next = self.node(h).next.load(Ordering::Acquire);
            // Перепроверка: head не сдвинулся -> node(h) всё ещё dummy, next консистентен.
            if h != self.head.load(Ordering::Acquire) {
                continue;
            }

            // Пусто <=> у dummy нет преемника.
            if next == NULL_INDEX {
                return None;
            }

            // Преемник есть. Если tail отстаёт (head == tail), помогаем продвинуть его,
            // затем повторяем.
            let t = self.tail.load(Ordering::Acquire);
            if h == t {
                let _ = self
                    .tail
                    .compare_exchange(t, next, Ordering::AcqRel, Ordering::Relaxed);
                pause();
                continue;
            }

            // Значение видно через Acquire-загрузку next выше (release/acquire по next).
            // Без реюза оно неизменно, поэтому читать можно и до CAS.
            let v = self.node(next).value.load(Ordering::Acquire);
            if self
                .head
                .compare_exchange(h, next, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Some(v);
            }
            // CAS не прошёл (head сдвинулся) - повтор.
            pause();
        }
    }
}

#[cfg(all(test, not(feature = "sanitizers")))]
mod tests {
    use super::*;

    #[test]
    fn test_fifo_single_thread() {
        let q = MSQueue::with_capacity(8);
        assert_eq!(q.dequeue(), None, "пустая очередь -> None");

        for v in 1..=5usize {
            assert_eq!(q.enqueue(v), Ok(()));
        }
        for v in 1..=5usize {
            assert_eq!(q.dequeue(), Some(v), "порядок FIFO");
        }
        assert_eq!(q.dequeue(), None, "снова пусто");
    }

    #[test]
    fn test_enqueue_full_returns_err() {
        // Без реюза ёмкость считается на всё время жизни: 2 элемента максимум.
        let q = MSQueue::with_capacity(2);
        assert_eq!(q.enqueue(10), Ok(()));
        assert_eq!(q.enqueue(20), Ok(()));
        assert_eq!(q.enqueue(30), Err(30), "пул исчерпан -> Err(value)");
        // Снятие НЕ возвращает слот в оборот (узлы не переиспользуются).
        assert_eq!(q.dequeue(), Some(10));
        assert_eq!(
            q.enqueue(30),
            Err(30),
            "слот не переиспользуется -> всё ещё Err"
        );
    }

    #[test]
    fn test_mpmc_no_loss_no_duplication() {
        use std::collections::HashSet;
        use std::sync::atomic::{AtomicUsize as Counter, Ordering as O};
        use std::sync::{Arc, Mutex};
        use std::thread;

        let per_producer = 100usize;
        let producers = 2usize;
        let total = per_producer * producers;

        // Ёмкость >= total, т.к. узлы не переиспользуются.
        let q = Arc::new(MSQueue::with_capacity(total));
        let collected = Arc::new(Mutex::new(Vec::<usize>::with_capacity(total)));
        let drained = Arc::new(Counter::new(0));

        let mut producer_handles = Vec::new();
        for p in 0..producers {
            let q = Arc::clone(&q);
            producer_handles.push(thread::spawn(move || {
                for k in 0..per_producer {
                    // Уникальные ненулевые значения: p*per + k + 1.
                    let val = p * per_producer + k + 1;
                    while q.enqueue(val).is_err() {
                        thread::yield_now();
                    }
                }
            }));
        }

        let mut consumer_handles = Vec::new();
        for _ in 0..2 {
            let q = Arc::clone(&q);
            let collected = Arc::clone(&collected);
            let drained = Arc::clone(&drained);
            consumer_handles.push(thread::spawn(move || {
                loop {
                    if drained.load(O::Acquire) >= total {
                        break;
                    }
                    if let Some(v) = q.dequeue() {
                        collected.lock().unwrap().push(v);
                        drained.fetch_add(1, O::AcqRel);
                    } else {
                        thread::yield_now();
                    }
                }
            }));
        }

        for h in producer_handles {
            h.join().unwrap();
        }
        for h in consumer_handles {
            h.join().unwrap();
        }

        let mut got = collected.lock().unwrap().clone();
        got.sort_unstable();
        let expected: Vec<usize> = (1..=total).collect();
        assert_eq!(got.len(), total, "число снятых элементов должно совпасть");
        let unique: HashSet<usize> = got.iter().copied().collect();
        assert_eq!(unique.len(), total, "дубликатов быть не должно");
        assert_eq!(got, expected, "должны получить ровно 1..=total без потерь");
    }
}

#[cfg(feature = "sanitizers")]
mod loom_tests {
    use super::*;
    use loom::sync::Arc;
    use loom::thread;

    /// Один продюсер и один консьюмер, один элемент. МАЛЕНЬКИЙ размер намеренно:
    /// MS-queue под loom комбинаторно тяжёлая. Проверяем, что helping-переход
    /// (консьюмер может продвинуть отстающий tail) не теряет элемент.
    #[test]
    fn loom_one_element_handoff() {
        loom::model(|| {
            let q = Arc::new(MSQueue::with_capacity(2));
            let qp = Arc::clone(&q);
            let qc = Arc::clone(&q);

            let producer = thread::spawn(move || {
                while qp.enqueue(42).is_err() {
                    loom::thread::yield_now();
                }
            });

            let consumer = thread::spawn(move || {
                loop {
                    if let Some(v) = qc.dequeue() {
                        assert_eq!(v, 42, "получено ровно то, что положили");
                        break;
                    }
                    loom::thread::yield_now();
                }
            });

            producer.join().unwrap();
            consumer.join().unwrap();
        });
    }
}
