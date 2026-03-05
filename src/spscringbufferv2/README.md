# SPSCRingBufferV2

**Single-Producer-Single-Consumer Ring Buffer с оптимизацией кэш-линий** (cache line optimized).

> Перед чтением этого документа рекомендуется ознакомиться с
> [README для SPSCRingBuffer](../spscringbuffer/README.md), где подробно обосновываются memory orders.
> Здесь описывается только суть оптимизации; memory orders используются те же.

## Проблема: cache line bouncing в V1

В `SPSCRingBuffer` (V1) каждый вызов `try_produce` делает `head.load(Ordering::Acquire)`, а каждый вызов
`try_consume` — `tail.load(Ordering::Acquire)`.

`head` пишет потребитель → после каждой записи (`head.store(Release)`) кэш-линия с `head` в ядре
производителя **инвалидируется** по протоколу MESI.

![mesi.png](../../assets/mesi.png)

```
     Производитель (ядро 0)               Потребитель (ядро 1)
   ┌──────────────────────┐            ┌──────────────────────┐
   │ L1: [head=5] INVALID │←─invalidate│ L1: [head=6] MODIFIED│
   │                      │            │                      │
   │ head.load(Acquire)   │──request──→│ flush → [head=6]     │
   │ (ждём данные...)     │←──data─────│                      │
   └──────────────────────┘            └──────────────────────┘
```

Такая «перекидка» кэш-линии между ядрами происходит **при каждом вызове** `try_produce`/`try_consume` в V1.
Задержка одного обращения к кэшу другого ядра — десятки-сотни наносекунд; при высоком throughput это
становится узким местом.

Аналогичная проблема симметрична: производитель пишет в `tail`, после чего `tail` в кэше потребителя
инвалидируется, и потребителю приходится ждать на каждом `tail.load(Acquire)`.

## Оптимизация: кэширование индексов

SPSCRingBufferV2 вводит два поля — **локальные копии** «чужих» индексов:

```rust
cached_head: Cell<usize>,  // кэш производителя: последнее известное значение head
cached_tail: Cell<usize>,  // кэш потребителя: последнее известное значение tail
```

Ключевая идея: **делать дорогой `Acquire`-load чужого индекса только тогда, когда без него не обойтись** —
то есть лишь в момент, когда по устаревшему кэшу буфер кажется полным (для производителя) или пустым
(для потребителя).

### `try_produce` (V2)

```rust
pub fn try_produce(&self, value: T) -> bool {
    let current_tail = self.tail.load(Ordering::Relaxed);       // (1)

    // Обновляем кэш, только если буфер *кажется* заполненным по кэшированному значению
    if self.next(current_tail) == self.cached_head.get() {      // (2)
        self.cached_head.set(self.head.load(Ordering::Acquire)); // (3)
    }

    let current_head = self.cached_head.get();                  // (4)

    if self.is_full(current_head, current_tail) {
        return false;
    }

    unsafe { ptr::write(self.slot_ptr(current_tail), value); }

    self.tail.store(self.next(current_tail), Ordering::Release); // (5)

    true
}
```

В **общем случае** (буфер не заполнен) шаг (3) пропускается. Производитель обходится:
- `tail.load(Relaxed)` — чтение собственного индекса, не затрагивает кэш-линию другого ядра;
- чтением `cached_head` из своей же кэш-линии.

`Acquire`-load на `head` (шаг 3) выполняется **только** когда `next(tail) == cached_head`, т. е. когда
кэшированное значение говорит, что буфер полон. Именно в этот момент и нужна актуальная информация.

### `try_consume` (V2)

```rust
pub fn try_consume(&self) -> Option<T> {
    let current_head = self.head.load(Ordering::Relaxed);        // (1)

    // Обновляем кэш, только если буфер *кажется* пустым по кэшированному значению
    if current_head == self.cached_tail.get() {                  // (2)
        self.cached_tail.set(self.tail.load(Ordering::Acquire)); // (3)
    }

    let current_tail = self.cached_tail.get();                   // (4)

    if self.is_empty(current_head, current_tail) {
        return None;
    }

    let value = unsafe { ptr::read(self.slot_ptr(current_head)) };

    self.head.store(self.next(current_head), Ordering::Release); // (5)

    Some(value)
}
```

Симметричная логика: `tail.load(Acquire)` (шаг 3) выполняется только когда `head == cached_tail`,
то есть когда кэшированное значение говорит, что буфер пуст.

## Memory Orders (V2)

Memory orders идентичны V1; обоснование см. в [README для SPSCRingBuffer](../spscringbuffer/README.md).

| Операция | Memory order | Роль |
|---|---|---|
| `tail.load` (производитель, шаг 1) | **Relaxed** | Производитель читает свой индекс; modification order достаточен |
| `head.load` (производитель, шаг 3) | **Acquire** | **synchronizes-with** `head.store(Release)` потребителя; устанавливает happens-before |
| `tail.store` (производитель, шаг 5) | **Release** | Публикует запись в буфер потребителю |
| `head.load` (потребитель, шаг 1) | **Relaxed** | Потребитель читает свой индекс; modification order достаточен |
| `tail.load` (потребитель, шаг 3) | **Acquire** | **synchronizes-with** `tail.store(Release)` производителя; устанавливает happens-before |
| `head.store` (потребитель, шаг 5) | **Release** | Освобождает слот буфера для производителя |

Отношения **synchronizes-with** → **happens-before** те же, что в V1:

![synchronizes-with.png](../../assets/synchronizes-with.png)

## Корректность кэширования

Кэшированное значение может быть **устаревшим**, но это безопасно.

### `cached_head` в производителе

Устаревший `cached_head` — это старое (меньшее) значение `head`. Производитель думает, что буфер
**более заполнен**, чем на самом деле.

- Производитель может сделать «лишний» `Acquire`-load (шаг 3) или вернуть `false`.
- Производитель **никогда не перезапишет слот, который потребитель ещё не прочитал**: перед фактическим
  решением о переполнении всегда выполняется актуальный `Acquire`-load.

### `cached_tail` в потребителе

Устаревший `cached_tail` — это старое (меньшее) значение `tail`. Потребитель думает, что буфер
**более пуст**, чем на самом деле.

- Потребитель может сделать «лишний» `Acquire`-load или вернуть `None`.
- Потребитель **никогда не прочитает неинициализированный слот**: перед фактическим решением об опустошении
  всегда выполняется актуальный `Acquire`-load.

Таким образом, устаревший кэш ухудшает лишь **производительность** в редких случаях, но не **корректность**.

## Сравнение V1 и V2

| Сценарий | V1: `Acquire`-loads на вызов | V2: `Acquire`-loads на вызов |
|---|---|---|
| Буфер не полон (producer) | 1 (`head`) | 0 |
| Буфер кажется полным (producer) | 1 | 1 (обновление кэша) |
| Буфер не пуст (consumer) | 1 (`tail`) | 0 |
| Буфер кажется пустым (consumer) | 1 | 1 (обновление кэша) |

В производственных системах с высоким throughput буфер редко достигает крайних состояний (полный / пустой).
V2 значительно снижает межъядерный трафик и количество операций ожидания по протоколу когерентности кэша.
