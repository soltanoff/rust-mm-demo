# SPSCRingBuffer

**Single-Producer-Single-Consumer Ring Buffer** — кольцевой буфер для взаимодействия ровно одного производителя
(**producer**) и одного потребителя (**consumer**).

## Структура

```
  head        tail
   ↓           ↓
 [ a | b | c | _ | _ ]
       ↑       ↑
    consumed  next write
```

- `buffer` — массив элементов фиксированной ёмкости
- `head` — индекс следующего элемента для чтения; **владелец: потребитель**
- `tail` — индекс следующей позиции для записи; **владелец: производитель**

Ключевой инвариант: каждый из двух индексов пишет ровно **один** поток. Это **фундаментальное** свойство SPSC,
на котором строится вся аргументация memory orders.

## Memory Orders

Паттерн взаимодействия производителя и потребителя — классический **message passing** через атомики
(см. раздел 
[Порядок - Message Passing (happens-before)](../../README.md#порядок---message-passing-happens-before)).

![message-passing-via-atomics.png](../../assets/message-passing-via-atomics.png)

Каждая пара `Release`-store / `Acquire`-load на атомике образует отношение **synchronizes-with**,
которое вместе с **program order** образует **happens-before** (см. разделы 
[15. Гарантии](../../README.md#15-гарантии) и
[16. Отношения и частичные порядки](../../README.md#16-отношения-и-частичные-порядки)).

![synchronizes-with.png](../../assets/synchronizes-with.png)

### `try_produce`

```rust
pub fn try_produce(&self, value: T) -> bool {
    let current_head = self.head.load(Ordering::Acquire); // (1)
    let current_tail = self.tail.load(Ordering::Relaxed); // (2)

    if self.is_full(current_head, current_tail) {
        return false;
    }

    unsafe { ptr::write(self.slot_ptr(current_tail), value); } // (3)

    self.tail.store(self.next(current_tail), Ordering::Release); // (4)

    true
}
```

| # | Операция            | Memory order | Обоснование                                                                                                                                                                                                                                                                                                                                   |
|---|---------------------|--------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| 1 | `head.load`         | **Acquire**  | `head` пишет потребитель с `Release` (шаг 8). `Acquire` здесь создаёт **synchronizes-with** -> **happens-before**: все действия потребителя до его `head.store(Release)` — в частности, чтение слота буфера — **happens-before** этого load. Без `Acquire` производитель мог бы перезаписать слот, который потребитель ещё не успел прочитать. |
| 2 | `tail.load`         | **Relaxed**  | `tail` пишет **только производитель**. Читая собственное значение, производитель всегда видит его последнюю запись благодаря гарантии **modification order** (присутствует даже у `Relaxed`). Дополнительная синхронизация не нужна: нет другого потока, чью запись в `tail` надо "увидеть".                                                  |
| 3 | `ptr::write` в слот | —            | Неатомарная запись данных. Безопасна, поскольку (1) установил **happens-before** с `head.store(Release)` потребителя: потребитель гарантированно закончил читать этот слот.                                                                                                                                                                   |
| 4 | `tail.store`        | **Release**  | Публикует факт записи данных потребителю. `Release` гарантирует: запись (3) не будет переупорядочена после этого store. Потребитель, прочитавший `tail` с `Acquire`, увидит актуальные данные в буфере.                                                                                                                                       |

### `try_consume`

```rust
pub fn try_consume(&self) -> Option<T> {
    let current_head = self.head.load(Ordering::Relaxed); // (5)
    let current_tail = self.tail.load(Ordering::Acquire); // (6)

    if self.is_empty(current_head, current_tail) {
        return None;
    }

    let value = unsafe { ptr::read(self.slot_ptr(current_head)) }; // (7)

    self.head.store(self.next(current_head), Ordering::Release); // (8)

    Some(value)
}
```

| # | Операция             | Memory order | Обоснование                                                                                                                                                                                                                                                                  |
|---|----------------------|--------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| 5 | `head.load`          | **Relaxed**  | `head` пишет **только потребитель**. Аналогично (2): потребитель читает собственное значение, **modification order** достаточен.                                                                                                                                                 |
| 6 | `tail.load`          | **Acquire**  | `tail` пишет производитель с `Release` (шаг 4). `Acquire` создаёт **synchronizes-with** -> **happens-before**: запись данных производителем (3) **happens-before** чтения (7). Без `Acquire` потребитель мог бы прочитать устаревшие или частично инициализированные данные. |
| 7 | `ptr::read` из слота | —            | Неатомарное чтение данных. Безопасно: (6) установил **happens-before** с `ptr::write` производителя.                                                                                                                                                                         |
| 8 | `head.store`         | **Release**  | Освобождает слот для производителя. `Release` гарантирует: чтение (7) не будет переупорядочено после этого store. Производитель, прочитавший `head` с `Acquire` (1), увидит, что потребитель уже прочитал данные, и сможет безопасно перезаписать слот.                      |

## Цепочки happens-before

```
Производитель (T0)                    Потребитель (T1)
───────────────────                 ────────────────────
   write(val) 
       ↓ (po)                               
tail.store(Release) ───── sw ────>  tail.load(Acquire)
                                           ↓ (po)
                                       read(slot)
                                           ↓ (po)
head.load(Acquire)  <──── sw ─────  head.store(Release)
       ↓ (po)
  write(newval)
```

Здесь `po` — **program order**, `sw` — **synchronizes-with**.

Каждая `sw`-связь вместе с `po`-связями образует полную цепочку **happens-before**, гарантируя:

1. Производитель видит данные в слоте только после того, как потребитель их прочитал.
2. Потребитель видит данные в слоте только после того, как производитель их записал.

## Почему Relaxed безопасен

Relaxed предоставляет только гарантию **modification order** — порядок записей в конкретный атомик наблюдается
согласованно (см. [раздел 19](../../README.md#19-слабые-модели-памяти)):

- `seq_cst`: **synchronization order** + **happens-before** + **modification order**
- `release` + `acquire`: ~~synchronization order~~ + **happens-before** + **modification order**
- `relaxed`: ~~synchronization order~~ + ~~happens-before~~ + **modification order**

В SPSC-буфере каждый из двух атомиков (`head`, `tail`) имеет ровно **одного писателя**. Поток-писатель
при чтении собственного атомика не нуждается в синхронизации с другим потоком — он просто видит собственные
же записи, и **modification order** это гарантирует.

Синхронизация с другим потоком осуществляется **через противоположный атомик** с парой `Release`/`Acquire`:

| Атомик | Писатель      | Читает свой:         | Читает чужой (Acquire):              |
|--------|---------------|----------------------|--------------------------------------|
| `tail` | Производитель | `head.load(Relaxed)` | `tail.load(Acquire)` у потребителя   |
| `head` | Потребитель   | `tail.load(Relaxed)` | `head.load(Acquire)` у производителя |
