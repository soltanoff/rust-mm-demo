# Test-And-TAS SpinLock

## Обзор

Реализация **Test-And-TAS** (TA-TAS) спинлока — оптимизированного варианта классического **TAS** (Test-And-Set) спинлока.

```rust
pub fn lock(&self) {
    // RMW операция
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
```

## Почему TA-TAS, а не TAS

Наивный TAS спинлок выглядит так:

```rust
fn lock(&self) {
    while self.locked.compare_exchange_weak(false, true, ...).is_err() {}
}
```

`compare_exchange_weak` — это **RMW**-операция (Read-Modify-Write). Каждый вызов в цикле пытается
**записать** в атомик, а запись в ячейку памяти в протоколе когерентности (например,
[MESI](../../assets/mesi.png)) требует захвата кэш-линии в **эксклюзивное** владение. Для этого
нужно инвалидировать кэш-линию во всех остальных кэшах. Если несколько ядер крутятся в TAS-цикле,
они непрерывно отнимают друг у друга кэш-линию — это чистая **коммуникация**, а коммуникация — это
простой ядра.

В TA-TAS добавляется внутренний цикл с **read-only** операцией `load`:

```rust
while self.locked.load(Ordering::Relaxed) {
    pause();
}
```

`load` — это **чтение**. Чтение не требует эксклюзивного владения кэш-линией — каждое ядро читает
из своего локального кэша в состоянии **Shared**. Когерентный трафик генерируется только в момент,
когда лок освобождается и кэш-линия обновляется. После этого ядра видят `locked == false` и
**только тогда** пытаются выполнить дорогую RMW-операцию.

## Выбор Memory Order

### `lock`: `compare_exchange_weak(false, true, Acquire, Relaxed)`

Спинлок реализует паттерн **message passing** через атомик `locked`
(см. [Message Passing via Atomics](../../assets/message-passing-via-atomics.png)):

- **Отправка сообщения** — `unlock()`, запись `false` в атомик.
- **Доставка сообщения** — `lock()`, чтение `false` из атомика через успешный `compare_exchange_weak`.

Когда `compare_exchange_weak` **успешно** меняет `false → true`, поток читает значение, записанное
предыдущим `unlock()`. Именно в этот момент между `unlock()` предыдущего владельца и `lock()`
нового владельца устанавливается отношение
[**synchronizes-with**](../../assets/synchronizes-with.png).

Из **synchronizes-with** через **program order** строится
[**happens-before**](../../assets/hb.png) — наблюдаемая программой причинность:

1. Все записи в **критическую секцию** потока-отправителя **sequenced-before** `unlock()`.
2. `unlock()` **synchronizes-with** успешный `lock()`.
3. Успешный `lock()` **sequenced-before** чтения в критической секции потока-получателя.
4. Транзитивное замыкание даёт **happens-before** между записями предыдущей и чтениями
   следующей критической секции.

Именно это и обеспечивает
[видимость](../../assets/visibility-hb.png): чтения в
критической секции наблюдают **последнюю** предшествующую в **happens-before** запись.

Для построения **synchronizes-with** достаточно пары **Release/Acquire** — полный
**synchronization order** (`SeqCst`) здесь не нужен. Мы не требуем глобального порядка на
всех атомиках — нам нужна только причинная связь между `unlock` и `lock` одного и того же
спинлока.

Таким образом, из
[иерархии гарантий](../../README.md#19-слабые-модели-памяти):

- `seq_cst`: **synchronization order** + **happens-before** + **modification order**
- `release` + `acquire`: ~~synchronization order~~ + **happens-before** + **modification order** ✅
- `relaxed`: ~~synchronization order~~ + ~~happens-before~~ + **modification order**

пара **Release/Acquire** — это оптимальный (самый слабый допустимый) уровень, обеспечивающий
корректность: **happens-before** гарантирует видимость записей из предшествующей критической
секции, а **modification order** гарантирует согласованный порядок захватов лока.

#### Почему `Acquire` именно на успешном пути

`Acquire` на **успешном** `compare_exchange_weak` означает: барьер нужен только когда мы
**действительно захватили** лок и нам пора читать разделяемое состояние. На пути неудачи
(`Relaxed`) барьер не нужен — мы не входим в критическую секцию и не обращаемся к защищённым
данным.

#### Почему `compare_exchange_weak`, а не `compare_exchange`

`compare_exchange_weak` допускает **spurious failure** — ложное срабатывание, при котором
операция возвращает ошибку, даже если текущее значение совпадает с ожидаемым. На некоторых
архитектурах (ARM, RISC-V) RMW-операции реализуются через пару `LL/SC` (Load-Linked /
Store-Conditional), и `SC` может не пройти из-за потери кэш-линии. `compare_exchange`
(«сильный» вариант) обязан замаскировать этот spurious failure внутренним retry-циклом, что
добавляет лишние инструкции. Поскольку наш `compare_exchange_weak` уже находится во внешнем
`while`-цикле, ложная неудача просто приведёт к следующей итерации — дополнительный
внутренний retry не нужен.

### `lock` (внутренний цикл): `load(Relaxed)`

```rust
while self.locked.load(Ordering::Relaxed) {
    pause();
}
```

Этот `load` — чисто оптимизационный: мы опрашиваем атомик, ожидая, когда он станет `false`.
Нам **не нужны** никакие гарантии видимости на этом этапе:

- Мы **не входим** в критическую секцию.
- Мы **не читаем** защищённое состояние.
- `Relaxed` гарантирует только **modification order** — мы не пропустим запись `false`, мы
  просто можем увидеть её с задержкой.

Как только `load` вернёт `false`, мы выйдем из внутреннего цикла и попробуем выполнить
`compare_exchange_weak` с `Acquire` — именно **там** будет установлен барьер и построена
цепочка **happens-before**.

### `unlock`: `store(false, Release)`

```rust
pub fn unlock(&self) {
    self.locked.store(false, Ordering::Release);
}
```

`Release` на записи означает: все записи в память, выполненные текущим потоком **до** этой
точки (**sequenced-before**, т.е. в **program order**), станут видимы потоку, который выполнит
парный `Acquire`-load этого значения.

Это вторая половина паттерна **message passing**:

| Поток-владелец (unlock)                                                         | Поток-захватчик (lock)                                                            |
|---------------------------------------------------------------------------------|-----------------------------------------------------------------------------------|
| Записи в критическую секцию <br/> `locked.store(false, Release)` — «отправка»   | `locked.CAS(false, true, Acquire)` — «доставка» <br/> Чтения в критическую секцию |

Между `store(Release)` и успешным `compare_exchange_weak(Acquire)` устанавливается
**synchronizes-with**, которое через транзитивность строит
[**happens-before**](../../assets/hb.png). Результат: чтения в новой
критической секции гарантированно видят все записи из предыдущей.

## Итого

| Операция | Memory Order | Причина |
|---|---|---|
| `compare_exchange_weak` (success) | `Acquire` | Строит **synchronizes-with** с парным `Release` из `unlock()` → обеспечивает **happens-before** → видимость записей предыдущей критической секции |
| `compare_exchange_weak` (failure) | `Relaxed` | Лок не захвачен — критическая секция не начинается, барьер не нужен |
| `load` (внутренний цикл) | `Relaxed` | Оптимизационный read-only опрос, без доступа к защищённым данным — достаточно **modification order** |
| `store` в `unlock()` | `Release` | Гарантирует видимость всех записей критической секции для потока, выполнившего парный `Acquire` |

Пара **Release/Acquire** — минимально необходимый и достаточный набор гарантий для корректной
работы спинлока в рамках [декларативной модели памяти](../../README.md#11-порядки-и-отношения).
Она обеспечивает **happens-before** между критическими секциями без накладных расходов полного
**synchronization order** (`SeqCst`).
