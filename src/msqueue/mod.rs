#![allow(dead_code)]

use crate::sync::{AtomicUsize, Ordering, pause};

// Упаковка "указателя" (index, version) в один usize.
// Предполагается 64-битная платформа: младшие 32 бита - индекс узла в пуле,
// старшие 32 - version-счётчик (защита от ABA). NULL кодируется индексом u32::MAX.
const INDEX_BITS: u32 = 32;
const INDEX_MASK: usize = (1usize << INDEX_BITS) - 1;
const NULL_INDEX: u32 = u32::MAX;

#[inline]
fn pack(index: u32, version: u32) -> usize {
    ((version as usize) << INDEX_BITS) | (index as usize)
}

#[inline]
fn index_of(packed: usize) -> u32 {
    (packed & INDEX_MASK) as u32
}

#[inline]
fn version_of(packed: usize) -> u32 {
    (packed >> INDEX_BITS) as u32
}

#[inline]
fn is_null(packed: usize) -> bool {
    index_of(packed) == NULL_INDEX
}

struct Node {
    // Payload АТОМАРНЫЙ: реюз узла делает возможной конкуренцию dequeue-чтения и
    // enqueue-записи по одному адресу. Обычное (не-атомарное) поле дало бы data race
    // (loom/Miri красные).
    // Атомик убирает UB. Консистентность значения обеспечивает тег на head (см. dequeue).
    // Именно поэтому очередь несёт usize, а не generic T.
    value: AtomicUsize,
    // Упакованный (index, version) следующего узла; index == NULL_INDEX -> конец списка.
    next: AtomicUsize,
}

/// Lock-free очередь Майкла–Скотта: helping + ABA (tagged pointers)
pub struct MSQueue {
    pool: Box<[Node]>, // фиксированная ёмкость, без реаллокации; доступ по индексу
    free: AtomicUsize, // голова freelist (Treiber-стек, tagged), упакованный указатель
    head: AtomicUsize, // упакованный указатель на dummy-узел
    tail: AtomicUsize, // упакованный указатель на последний (или предпоследний) узел
}
// Все поля атомарны -> MSQueue авто-Send + Sync; явная реализация не требуется.

impl MSQueue {
    /// Создаёт очередь ёмкостью `capacity` элементов. Выделяет `capacity + 1` узлов
    /// (один всегда занят dummy). Ёмкость фиксирована: `enqueue` при исчерпании пула
    /// возвращает `Err(value)`.
    pub fn with_capacity(capacity: usize) -> Self {
        assert!(capacity >= 1, "ёмкость должна быть >= 1");
        let total = capacity + 1; // +1 под dummy

        let mut nodes = Vec::with_capacity(total);
        for _ in 0..total {
            nodes.push(Node {
                value: AtomicUsize::new(0),
                next: AtomicUsize::new(pack(NULL_INDEX, 0)),
            });
        }
        let pool = nodes.into_boxed_slice();

        // Узел 0 - стартовый dummy (next уже = NULL).
        // Узлы 1..total связываем в односвязный freelist.
        for i in 1..total {
            let next_idx = if i + 1 < total {
                (i + 1) as u32
            } else {
                NULL_INDEX
            };
            pool[i].next.store(pack(next_idx, 0), Ordering::Relaxed);
        }
        let free_head = pack(1, 0); // total >= 2, значит узел 1 свободен

        Self {
            pool,
            free: AtomicUsize::new(free_head),
            head: AtomicUsize::new(pack(0, 0)),
            tail: AtomicUsize::new(pack(0, 0)),
        }
    }

    #[inline]
    fn node(&self, index: u32) -> &Node {
        &self.pool[index as usize]
    }

    /// Снять узел из freelist (Treiber pop). Тег на голове freelist растёт на каждом
    /// успешном pop - это защита самого freelist от ABA. `None` = пул исчерпан.
    fn alloc(&self) -> Option<u32> {
        loop {
            let head = self.free.load(Ordering::Acquire);
            if is_null(head) {
                return None;
            }
            let idx = index_of(head);
            let next = self.node(idx).next.load(Ordering::Acquire);
            let new_head = pack(index_of(next), version_of(head).wrapping_add(1));
            if self
                .free
                .compare_exchange(head, new_head, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Some(idx);
            }
            pause();
        }
    }

    /// Вернуть узел в freelist (Treiber push). Связываем узел на текущую голову
    /// (Relaxed), затем публикуем через CAS с Release: тег головы растёт (ABA freelist).
    ///
    /// ВАЖНО: версию поля `next` этого узла МОНОТОННО увеличиваем (не обнуляем).
    /// Тег на attach-CAS в `enqueue` защищает от ABA только если версия `next`
    /// растёт и переживает реюз узла; обнуление здесь сделало бы тег вырожденным.
    /// Узел в этот момент эксклюзивно наш (только что убран из очереди), поэтому
    /// `load`+`store` версии гонко-безопасны (атомики) и монотонны.
    fn free_node(&self, index: u32) {
        let ver = version_of(self.node(index).next.load(Ordering::Relaxed)).wrapping_add(1);
        loop {
            let head = self.free.load(Ordering::Acquire);
            // Узел ещё не достижим из freelist -> Relaxed достаточно; публикация
            // произойдёт Release-частью успешного CAS ниже.
            self.node(index)
                .next
                .store(pack(index_of(head), ver), Ordering::Relaxed);
            let new_head = pack(index, version_of(head).wrapping_add(1));
            if self
                .free
                .compare_exchange(head, new_head, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return;
            }
            pause();
        }
    }

    /// Поставить элемент в очередь. Два шага: (1) пристегнуть узел к `tail.next`,
    /// (2) продвинуть `tail`. Между ними `tail` отстаёт - тут и работает helping.
    pub fn enqueue(&self, value: usize) -> Result<(), usize> {
        let new_idx = match self.alloc() {
            Some(i) => i,
            None => return Err(value), // пул фиксированной ёмкости исчерпан
        };
        // Готовим узел. Публикация произойдёт Release-частью CAS над next ниже,
        // поэтому здесь достаточно Relaxed. Версию `next` МОНОТОННО увеличиваем
        // (не обнуляем) - иначе тег на attach-CAS вырождается и не ловит ABA при
        // реюзе. Узел только что снят с freelist -> эксклюзивно наш.
        self.node(new_idx).value.store(value, Ordering::Relaxed);
        let new_ver = version_of(self.node(new_idx).next.load(Ordering::Relaxed)).wrapping_add(1);
        self.node(new_idx)
            .next
            .store(pack(NULL_INDEX, new_ver), Ordering::Relaxed);

        loop {
            let t = self.tail.load(Ordering::Acquire);
            let t_next = self.node(index_of(t)).next.load(Ordering::Acquire);
            // Перепроверка: tail не сдвинулся между двумя загрузками.
            if t != self.tail.load(Ordering::Acquire) {
                continue;
            }

            if is_null(t_next) {
                // Пытаемся пристегнуть новый узел. Release публикует value/next узла.
                let attach = pack(new_idx, version_of(t_next).wrapping_add(1));
                if self
                    .node(index_of(t))
                    .next
                    .compare_exchange(t_next, attach, Ordering::Release, Ordering::Relaxed)
                    .is_ok()
                {
                    // best-effort: двигаем tail на новый узел (промах не критичен -
                    // помогут другие потоки).
                    let _ = self.tail.compare_exchange(
                        t,
                        pack(new_idx, version_of(t).wrapping_add(1)),
                        Ordering::AcqRel,
                        Ordering::Relaxed,
                    );
                    return Ok(());
                }
            } else {
                // HELPING: кто-то пристегнул узел, но не сдвинул tail. Помогаем.
                let _ = self.tail.compare_exchange(
                    t,
                    pack(index_of(t_next), version_of(t).wrapping_add(1)),
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                );
            }
            pause();
        }
    }

    /// Снять элемент.
    ///
    /// ВАЖНО (модель памяти): пустота определяется ТОЛЬКО по `next` (есть ли у dummy
    /// преемник), а НЕ по сравнению `head`/`tail`. Под слабой моделью памяти `Acquire`-
    /// загрузка `tail` вправе вернуть УСТАРЕВШЕЕ значение из modification order, поэтому
    /// `index(head) != index(tail)` НЕ означает непустоту (это несогласованный срез,
    /// невозможный лишь при sequential consistency). Единственный источник истины о
    /// пустоте - `next`. `tail` используем только чтобы решить, надо ли помочь его
    /// продвинуть; `next` разыменовываем лишь после проверки `next != NULL`.
    ///
    /// Значение читаем ДО CAS над `head`; монотонный тег `head` гарантирует, что при
    /// успешном CAS прочитанное значение относится к "тому самому" узлу (узел `next` не
    /// мог быть освобождён/переиспользован, пока `head` не двигался).
    pub fn dequeue(&self) -> Option<usize> {
        loop {
            let h = self.head.load(Ordering::Acquire);
            let next = self.node(index_of(h)).next.load(Ordering::Acquire);
            // Перепроверка: head не сдвинулся -> node(h) всё ещё dummy, next консистентен.
            if h != self.head.load(Ordering::Acquire) {
                continue;
            }

            // Пусто тогда и только тогда, когда у dummy нет преемника.
            if is_null(next) {
                return None;
            }

            // Преемник есть. Если tail отстаёт (head == tail), помогаем продвинуть его
            // ПЕРЕД снятием, чтобы head не обогнал tail, затем повторяем.
            let t = self.tail.load(Ordering::Acquire);
            if index_of(h) == index_of(t) {
                let _ = self.tail.compare_exchange(
                    t,
                    pack(index_of(next), version_of(t).wrapping_add(1)),
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                );
                pause();
                continue;
            }

            // head != tail и преемник есть -> безопасно снять. Значение читаем ДО CAS.
            let next_idx = index_of(next);
            let v = self.node(next_idx).value.load(Ordering::Acquire);
            if self
                .head
                .compare_exchange(
                    h,
                    pack(next_idx, version_of(h).wrapping_add(1)),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                // Победили: старый dummy возвращаем в пул, отдаём значение.
                self.free_node(index_of(h));
                return Some(v);
            }
            // CAS не прошёл (head сдвинулся) - прочитанное v отбрасываем, повтор.
            pause();
        }
    }
}

#[cfg(all(test, not(feature = "sanitizers")))]
mod tests {
    use super::*;

    #[test]
    fn test_pack_unpack_roundtrip() {
        let p = pack(7, 42);
        assert_eq!(index_of(p), 7);
        assert_eq!(version_of(p), 42);
        assert!(!is_null(p));
        assert!(is_null(pack(NULL_INDEX, 3)));
    }

    /// ABA: индекс вернулся тот же, но поколение выросло.
    /// Сравнение только по индексу пропустило бы подмену; сравнение по
    /// упакованному (index, version) - нет. Это ядро защиты tagged pointers.
    #[test]
    fn test_tag_detects_aba_that_index_only_would_miss() {
        // Поток A прочитал упакованный указатель на узел 3, поколение 5.
        let observed = pack(3, 5);
        // Поток B: dequeue(3) -> free -> enqueue переиспользовал индекс 3,
        // поколение выросло до 7. head снова "указывает на 3".
        let actual = pack(3, 7);

        // Только по индексу: ABA НЕ замечена -> ложный успех CAS.
        assert_eq!(index_of(observed), index_of(actual));
        // По упакованному значению: ABA замечена -> CAS(expected=observed) провалится.
        assert_ne!(observed, actual);
    }

    #[test]
    fn test_freelist_alloc_then_free_reuses_index() {
        let q = MSQueue::with_capacity(2); // total = 3 узла: 0 - dummy, 1 и 2 - свободны
        let a = q.alloc().expect("должен быть свободный узел");
        let b = q.alloc().expect("должен быть свободный узел");
        assert_ne!(a, b, "два alloc дают разные индексы");
        assert_eq!(q.alloc(), None, "пул исчерпан (dummy занимает один узел)");

        q.free_node(a);
        let c = q.alloc().expect("после free узел снова доступен");
        assert_eq!(
            c, a,
            "freelist (LIFO) переиспользует только что освобождённый индекс"
        );
    }

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
        let q = MSQueue::with_capacity(2); // вмещает 2 элемента
        assert_eq!(q.enqueue(10), Ok(()));
        assert_eq!(q.enqueue(20), Ok(()));
        assert_eq!(q.enqueue(30), Err(30), "пул исчерпан -> Err(value)");
        // Освободили место -> снова можно.
        assert_eq!(q.dequeue(), Some(10));
        assert_eq!(q.enqueue(30), Ok(()));
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

        let q = Arc::new(MSQueue::with_capacity(256));
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
                        thread::yield_now(); // очередь временно полна
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
