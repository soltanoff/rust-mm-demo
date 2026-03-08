# Lazy Initializer

## Обзор

Реализация потокобезопасного **Lazy Initializer** — структуры, которая гарантирует, что
инициализация значения произойдёт **ровно один раз**, а все последующие обращения получат
указатель на уже инициализированный объект. В основе лежит паттерн **Double-Checked Locking (DCL)**.

```rust
pub fn access(&self) -> &T {
    // Fast path
    let mut ptr = self.cell.load(Ordering::Acquire);     // (1)

    // Slow path
    if ptr.is_null() {
        self.mutex.lock();                               // (2)

        // Double-check после захвата mutex
        ptr = self.cell.load(Ordering::Relaxed);         // (3)
        if ptr.is_null() {
            unsafe {
                ptr = Box::into_raw(Box::new((*self.init.get())()));
                self.cell.store(ptr, Ordering::Release); // (4)
            }
        }

        self.mutex.unlock();                             // (5)
    }

    unsafe { &*ptr }
}
```

## Паттерн Double-Checked Locking

DCL — оптимизация, которая позволяет избежать захвата мьютекса на **горячем пути** (когда
значение уже инициализировано). Идея проста:

1. **Fast path** — атомарно читаем указатель. Если он не `null`, значение уже готово — возвращаем
   его **без** захвата мьютекса.
2. **Slow path** — указатель `null`, значит значение ещё не инициализировано (или инициализируется
   прямо сейчас другим потоком). Захватываем мьютекс и проверяем **ещё раз** (double-check):
   может быть, пока мы ждали лок, другой поток уже всё инициализировал.

## Выбор Memory Orders

### (1) Fast path: `cell.load(Ordering::Acquire)`

Это ключевая точка синхронизации на горячем пути. Когда `load` возвращает ненулевой указатель,
он читает значение, записанное `store(ptr, Release)` из операции `(4)`. Между ними
устанавливается отношение **synchronizes-with**:

![synchronizes-with.png](../../assets/synchronizes-with.png)

Из **synchronizes-with** через **program order** (sequenced-before) строится цепочка
[**happens-before**](../../assets/hb.png):

1. Вызов `init()` и аллокация объекта через `Box::new` **sequenced-before** `store(ptr, Release)`.
2. `store(ptr, Release)` **synchronizes-with** `load(Acquire)`.
3. `load(Acquire)` **sequenced-before** разыменование указателя `&*ptr`.
4. Транзитивное замыкание даёт **happens-before** между инициализацией объекта и его чтением.

Именно это обеспечивает **видимость**: когда поток разыменовывает `ptr`, он гарантированно
видит **полностью инициализированный** объект — последнюю предшествующую в **happens-before**
запись (см. раздел [15. Гарантии](../../README.md#15-гарантии)).

![visibility-hb.png](../../assets/visibility-hb.png)

Без `Acquire` здесь мы бы потеряли **happens-before** — поток мог бы увидеть ненулевой указатель,
но при разыменовании наблюдать **частично инициализированный** объект. Это классический
баг сломанного DCL, знаменитый в мире Java/C++.

Из [иерархии гарантий](../../README.md#19-слабые-модели-памяти):

- `seq_cst`: **synchronization order** + **happens-before** + **modification order**
- `release` + `acquire`: ~~synchronization order~~ + **happens-before** + **modification order** ✅
- `relaxed`: ~~synchronization order~~ + ~~happens-before~~ + **modification order**

`Acquire` — минимально необходимый порядок: нам нужен **happens-before**.

### (4) Публикация: `cell.store(ptr, Ordering::Release)`

`Release` на записи означает: все записи в память, выполненные текущим потоком **до** этой
точки (**sequenced-before**, т.е. в **program order**), станут видимы потоку, который
выполнит парный `Acquire`-load этого значения.

Это **отправка сообщения** в паттерне [message passing](../../README.md#порядок---message-passing-happens-before):

![message-passing-via-atomics.png](../../assets/message-passing-via-atomics.png)

| Поток-инициализатор (store)                                            | Поток-читатель (load)                                          |
|------------------------------------------------------------------------|----------------------------------------------------------------|
| `init()` + `Box::new(...)` <br/> `cell.store(ptr, Release)` — отправка | `cell.load(Acquire)` — доставка <br/> `&*ptr` — чтение объекта |

Между `store(Release)` и `load(Acquire)` устанавливается **synchronizes-with**, которое
через транзитивность строит [**happens-before**](../../assets/hb.png). Как итог - чтение
объекта через `&*ptr` гарантированно видит все записи, выполненные при инициализации.

### (3) Double-check внутри мьютекса: `cell.load(Ordering::Relaxed)`

```rust
self.mutex.lock();
ptr = self.cell.load(Ordering::Relaxed); // (3)
```

Это самый интересный момент в реализации. Почему здесь достаточно `Relaxed`, хотя мы
тоже читаем указатель и потом его разыменовываем? А просто потому что **happens-before** уже обеспечен мьютексом.

Мьютекс (SpinLock) реализован на паре `Acquire`/`Release` (см. [SpinLock README](../spinlock/README.md)). 
Если поток A инициализировал объект и вышел из мьютекса, а поток B потом захватил мьютекс, то между `unlock()` 
потока A и `lock()` потока B устанавливается **synchronizes-with**, и через транзитивность — полная цепочка
**happens-before**:

![atomics-and-mutex.png](../../assets/atomics-and-mutex.png)

1. `init()` + `Box::new(...)` + `cell.store(ptr, Release)` **sequenced-before** `mutex.unlock()`.
2. `mutex.unlock()` **synchronizes-with** `mutex.lock()` потока B.
3. `mutex.lock()` **sequenced-before** `cell.load(Relaxed)` потока B.
4. Транзитивное замыкание: инициализация **happens-before** `cell.load(Relaxed)`.

Поскольку **happens-before** уже построен через мьютекс, `load` на `cell` не обязан
формировать собственную пару `Release`/`Acquire` — ему достаточно **modification order**
(единственная гарантия `Relaxed`), чтобы прочитать **актуальное** значение указателя.

> [!NOTE]
> `Relaxed` гарантирует **modification order** — порядок на всех записях в данный атомик.
> Это значит, что `load(Relaxed)` не может "пропустить" уже произошедшую запись, если она
> видима через **happens-before**. А она видима — через мьютекс.

Ставить здесь `Acquire` было бы **избыточно**: это лишние барьерные инструкции на целевой архитектуре, которые не дают
никаких дополнительных гарантий в данном контексте.

### (2) и (5): `mutex.lock()` / `mutex.unlock()`

Memory orders мьютекса подробно описаны в [SpinLock README](../spinlock/README.md).
Вкратце: `lock()` = `Acquire`, `unlock()` = `Release`. Пара образует **synchronizes-with**
между критическими секциями разных потоков — стандартный паттерн
[message passing](../../README.md#порядок---message-passing-happens-before).

## О `Consume` и dependency-ordered before

> [!Warning]
> Не используйте `consume`! С большой долей вероятности он вам не нужен. Он делает только хуже - перегружает определение 
> **happens-before**, добавляя в него консвенность **inter-thread happens before**. Держать в голове все частичные 
> порядки и отношения и так трудно, с `consume` становится еще сложнее. 
> Подробнее о проблематике [ниже](#почему-consume-не-используется).

В коде на строке `(1)` есть комментарий: `// Здесь мог быть Consume :)`

Это отсылка к `memory_order_consume` из C++ — ордерингу, который **теоретически** мог бы
быть здесь ещё более оптимальным, чем `Acquire`.

### Что такое `Consume`

В [стандарте C++](https://en.cppreference.com/w/cpp/atomic/memory_order.html) определён `memory_order_consume`, который
слабее `Acquire`, но сильнее `Relaxed`. 

`Consume` гарантирует видимость только тех записей, от которых **зависит** прочитанное значение. 
Это **dependency-ordered before** — более узкое отношение.

#### Порядок - Carries a dependency (несёт зависимость)

Оценка выражения `A` **carries a dependency to** оценки `B`, если:

- значение `A` используется как операнд `B` (кроме некоторых случаев с `std::kill_dependency`), **или**
- `A` записывает в скалярный объект `M`, `B` читает из `M`, и `A` **sequenced-before** `B`, **или**
- цепочка таких зависимостей транзитивно замыкается.

Иначе говоря, **carries a dependency** — это цепочка **зависимостей по данным** (data dependency)
внутри одного потока, где значение одной операции "протекает" в операнд следующей.

В нашем случае:

```rust
ptr = self.cell.load(...)   // A: загрузка указателя
&*ptr                       // B: разыменование указателя
```

`A` **carries a dependency to** `B`, потому что значение `ptr`, прочитанное из атомика,
используется как операнд при разыменовании.

#### Порядок - Dependency-Ordered Before

Оценка `A` **dependency-ordered before** оценки `B`, если `A` выполняет `release`-store над атомиком `M`, а 
в **другом** потоке `B` выполняет `consume`-load из `M` и читает значение, записанное `A` 
(или позднее в **modification order**).

Далее, если `A` **dependency-ordered before** `B`, а `B` **carries a dependency to** `C`,
то `A` **dependency-ordered before** `C`.

В C++ `dependency-ordered before` входит в определение **inter-thread happens before**, а тот —
в определение **happens-before** 
([6.9.2.2 [intro.races]](https://eel.is/c++draft/intro.races) + [memory_order](https://en.cppreference.com/w/cpp/atomic/memory_order.html)):

> An evaluation A happens before an evaluation B (or, equivalently, B happens after A) if either
> - A is sequenced before B, or
> - A synchronizes with B, or
> - A happens before X and X happens before B.
> 
> An evaluation A **inter-thread happens before** an evaluation B if:
> - A **synchronizes-with** B, or
> - A is **dependency-ordered before** B, or
> - ...транзитивные замыкания с sequenced-before...

Таким образом, `dependency-ordered before` — это **облегчённый** путь в **happens-before**:
он не требует полного барьера, а лишь сохранения цепочки зависимостей по данным.

### Почему `Consume` мог бы быть здесь

В нашем `access()` зависимость по данным **явная**: мы читаем **указатель** из атомика и тут
же его **разыменовываем**. Нам не нужна видимость **всех** записей предшествующего потока —
нам нужно только, чтобы данные, на которые **указывает** `ptr`, были видимы. А это ровно
то, что гарантирует `Consume` через **dependency-ordered before** + **carries a dependency**.

На архитектурах с **слабой моделью памяти** (ARM, POWER PC) `Consume` мог бы компилироваться в
**менее дорогие** инструкции, чем `Acquire`:

- `Acquire` на ARM -> `ldar` (load-acquire) или `dmb ishld` — полный барьер чтений.
- `Consume` на ARM -> обычный `ldr` + сохранение зависимости по данным.

Процессоры ARM и POWER PC **естественно** уважают зависимости по данным (address dependency): они **не** 
переупорядочивают загрузку значения и последующую загрузку **по адресу**, полученному из этого значения. 
Это свойство называется **address-dependency ordering** и поддерживается аппаратно без каких-либо барьеров.

### Почему `Consume` не используется

Несмотря на теоретическую привлекательность, `memory_order_consume` в C++ имеет статус
**(deprecated in C++26)**:

> [!NOTE]
> The specification of release and consume is intended to allow efficient multi-threaded producer-consumer patterns. 
> **Implementations are currently discouraged from relying on its semantic guarantees**.

Причины:

1. **Сложность формализации.** Понятие **carries a dependency** трудно точно отслеживать через
   произвольный код: оптимизации компилятора (constant propagation, CSE, register allocation)
   могут **разрушить** цепочку зависимостей по данным, хотя семантически она сохраняется.
2. **Компиляторы не реализуют.** На практике и GCC, и Clang, и MSVC **промотируют** `consume`
   до `acquire` — генерируют тот же барьер. Выигрыша в производительности нет.
3. **Нет в Rust.** `std::sync::atomic::Ordering` в Rust не содержит варианта `Consume`.
   [Rust наследует ММ C++20](https://doc.rust-lang.org/nomicon/atomics.htm), но сознательно исключает 
   `Consume` как непрактичный.

> [!NOTE]
> Rust pretty blatantly just **inherits the memory model for atomics from C++20**. This is not due to this model being
> particularly excellent or easy to understand. Indeed, this model is quite complex and known to have several flaws.
> Rather, it is a pragmatic concession to the fact that **everyone is pretty bad** at modeling atomics. At very least,
> we can
> benefit from existing tooling and research around the C/C++ memory model. (You'll often see this model referred to
> as "
> C/C++11" or just "C11". C just copies the C++ memory model; and C++11 was the first version of the model but it has
> received some bugfixes since then.)

Поэтому в коде стоит `Acquire` — это корректный, портируемый и единственный доступный в Rust вариант, который 
обеспечивает полный **happens-before**.

## Итого

| Операция                   | Memory Order | Причина                                                                                                                                                   |
|----------------------------|--------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------|
| `cell.load` (fast path)    | `Acquire`    | Строит **synchronizes-with** с парным `Release` из `store` -> **happens-before** -> видимость полностью инициализированного объекта при разыменовании `ptr` |
| `cell.store` (публикация)  | `Release`    | Гарантирует видимость всех записей инициализации (`init()` + `Box::new`) для потока, выполнившего парный `Acquire`                                        |
| `cell.load` (double-check) | `Relaxed`    | **happens-before** уже обеспечен мьютексом (`lock`=Acquire / `unlock`=Release) — достаточно **modification order**, чтобы прочитать актуальное значение   |
| `mutex.lock()`             | `Acquire`    | Стандартная семантика захвата мьютекса — см. [SpinLock](../spinlock/README.md)                                                                            |
| `mutex.unlock()`           | `Release`    | Стандартная семантика освобождения мьютекса — см. [SpinLock](../spinlock/README.md)                                                                       |

Пара **Release/Acquire** — минимально необходимый и достаточный набор гарантий для корректного
DCL в рамках [декларативной модели памяти](../../README.md#11-порядки-и-отношения).

`Relaxed` внутри мьютекса — безопасная оптимизация, т.к. **happens-before** обеспечивается
внешней синхронизацией. А `Consume`, хотя и был бы теоретически оптимальнее `Acquire` на
fast path, на практике недоступен в Rust.
