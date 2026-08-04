#![allow(dead_code)]

use crate::sync::{AtomicUsize, Ordering, pause};
use std::cell::UnsafeCell;

// "Нет узла" / конец списка. Очередь НЕ переиспользует узлы (bump-аллокация),
// поэтому индекс достаточно хранить как есть, без version-тега: раз узел никогда не
// возвращается в пул, ABA на head/tail/next невозможна и тегам нечего защищать.
const NULL_INDEX: usize = u32::MAX as usize;

struct Node<T> {
    // Payload с move-семантикой. Без реюза узла value пишется ровно один раз (enqueue,
    // до публикации узла) и вынимается ровно один раз (dequeue, после победы CAS) - обе
    // операции эксклюзивны, гонки нет. Для произвольного `T` нет `Atomic<T>`, поэтому
    // внутреннюю изменяемость даёт `UnsafeCell` (доступ к нему - `unsafe`). Синхронизацию
    // видимости обеспечивает цепочка release/acquire на поле `next`.
    value: UnsafeCell<Option<T>>,
    // Индекс следующего узла; NULL_INDEX -> конец "списка".
    next: AtomicUsize,
}

/// Lock-free MPMC-очередь Майкла-Скотта с механикой **helping**.
///
/// Узлы НЕ переиспользуются: enqueue берёт свежий индекс из пула bump-аллокатором, а
/// dequeue снятые узлы не освобождает. Это делает очередь корректной на `acq_rel` без
/// hazard pointers: раз узел никогда не возвращается в пул, ABA и use-after-free
/// невозможны, а version-теги на указателях не нужны.
///
/// Payload generic (`T`) с move-семантикой: значение вынимается из узла (`Option::take`)
/// только победителем CAS над `head`, поэтому доступ к нему эксклюзивен и move безопасен -
/// именно потому, что узлы не переиспользуются (иначе `next` мог бы быть освобождён и
/// переиспользован между победой и `take`).
///
/// Цена - ограниченная ёмкость на ВСЁ ВРЕМЯ ЖИЗНИ: снятые слоты не возвращаются в
/// оборот. Реальный реюз узлов потребовал бы безопасного reclamation (hazard pointers /
/// epoch), см. README.
pub struct MSQueue<T> {
    pool: Box<[Node<T>]>, // фиксированная ёмкость, без реаллокации. Доступ по индексу
    bump: AtomicUsize,    // следующий невыданный индекс пула (bump-аллокатор)
    head: AtomicUsize,    // индекс dummy-узла
    tail: AtomicUsize,    // индекс последнего (или предпоследнего) узла
}

// Доступ к `value` через `UnsafeCell` синхронизирован release/acquire на `next` и
// эксклюзивен (запись до публикации, вынос - после победы CAS). Для очереди,
// перемещающей `T` между потоками, достаточно `T: Send`.
unsafe impl<T: Send> Send for MSQueue<T> {}
unsafe impl<T: Send> Sync for MSQueue<T> {}

impl<T> MSQueue<T> {
    /// Создаёт очередь на не более чем `capacity` элементов ЗА ВСЁ ВРЕМЯ ЖИЗНИ (узлы
    /// не переиспользуются). Выделяет `capacity + 1` узлов: индекс 0 - стартовый dummy,
    /// индексы 1..=capacity выдаёт bump-аллокатор под элементы.
    pub fn with_capacity(capacity: usize) -> Self {
        assert!(capacity >= 1, "ёмкость должна быть >= 1");
        let total = capacity + 1; // +1 под dummy (индекс 0)

        let mut nodes = Vec::with_capacity(total);
        for _ in 0..total {
            nodes.push(Node {
                value: UnsafeCell::new(None),
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
    fn node(&self, index: usize) -> &Node<T> {
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

    /// Поставить элемент. Два шага:
    /// 1) пристегнуть узел к `tail.next`
    /// 2) продвинуть`tail`.
    /// Между ними `tail` отстаёт - тут и работает helping.
    pub fn enqueue(&self, value: T) -> Result<(), T> {
        let new_idx = match self.alloc() {
            Some(i) => i,
            None => return Err(value), // пул исчерпан на всё время жизни
        };
        // Узел только что выдан bump-аллокатором и ещё не опубликован -> эксклюзивно наш.
        // Пишем значение + публикация (видимость для читателей) произойдёт Release-частью
        // attach-CAS над next ниже.
        unsafe {
            *self.node(new_idx).value.get() = Some(value);
        }
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
    ///
    /// Значение вынимаем `take`-ом ТОЛЬКО после победы CAS над `head`: победитель один,
    /// а без реюза узел `next` не может быть освобождён/переиспользован, поэтому доступ к
    /// его `value` эксклюзивен и move безопасен. Видимость записи `value` обеспечивает
    /// Acquire-загрузка `next` выше (release/acquire по next).
    pub fn dequeue(&self) -> Option<T> {
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

            if self
                .head
                .compare_exchange(h, next, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                // Победили: node(next) стал новым dummy, только мы вынимаем его значение.
                // take() оставляет None; повторного выноса этого узла уже не будет.
                return unsafe { (*self.node(next).value.get()).take() };
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
        let q: MSQueue<usize> = MSQueue::with_capacity(8);
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
        let q: MSQueue<i32> = MSQueue::with_capacity(2);
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

    /// Move-семантика: payload не обязан быть `Copy`. Кладём и снимаем `String`.
    #[test]
    fn test_move_non_copy_payload() {
        let q: MSQueue<String> = MSQueue::with_capacity(4);
        q.enqueue("hello".to_string()).unwrap();
        q.enqueue("world".to_string()).unwrap();
        assert_eq!(q.dequeue(), Some("hello".to_string()));
        assert_eq!(q.dequeue(), Some("world".to_string()));
        assert_eq!(q.dequeue(), None);
    }

    /// Каждое значение дропается ровно один раз: снятое - когда его роняет получатель,
    /// не снятое (оставшееся в очереди) - при drop очереди. Ни утечки, ни двойного drop.
    #[test]
    fn test_drops_each_value_once() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize as Cnt, Ordering as O};

        let drops = Arc::new(Cnt::new(0));

        struct Guard(Arc<Cnt>);
        impl Drop for Guard {
            fn drop(&mut self) {
                self.0.fetch_add(1, O::SeqCst);
            }
        }

        {
            let q: MSQueue<Guard> = MSQueue::with_capacity(4);
            // Guard не Debug, поэтому не .unwrap(): проверяем is_ok().
            assert!(q.enqueue(Guard(Arc::clone(&drops))).is_ok());
            assert!(q.enqueue(Guard(Arc::clone(&drops))).is_ok());

            // Снимаем один и роняем его.
            let taken = q.dequeue();
            assert!(taken.is_some());
            drop(taken);
            assert_eq!(drops.load(O::SeqCst), 1, "снятый дропнут ровно раз");
            // Второй остаётся в очереди -> дропнется при drop(q) ниже.
        }
        assert_eq!(
            drops.load(O::SeqCst),
            2,
            "не снятый дропнут при drop очереди"
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
        let q: Arc<MSQueue<usize>> = Arc::new(MSQueue::with_capacity(total));
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
            let q: Arc<MSQueue<usize>> = Arc::new(MSQueue::with_capacity(2));
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
