# SharedPtr (V2 оптимизированный, как Arc)

Оптимизированная версия SharedPtr. В отличие от V1 (AcqRel везде), ordering'и ослаблены
до минимально необходимых - аналогично тому, как `std::sync::Arc` реализован в Rust stdlib.

## Идея оптимального упорядочивания в подсчете ссылок

Последний `fetch_sub` входит во все **release sequences** за счёт чего можно обеспечить согласованный `Acquire`.

> [!Note]
> An atomic operation `A` that is a release operation on an atomic object `M` **synchronizes with** an acquire fence 
> `B` if there exists some atomic operation `X` on `M` such that `X` is **sequenced before** `B` and reads the value 
> written by `A` or a value written by any **side effect** in the release **sequence headed by** `A`.

Source: https://eel.is/c++draft/atomics.fences#4

## Изменения относительно V1

| Операция   | V1                     | V2                               | Экономия                          |
| ---------- | ---------------------- | -------------------------------- | --------------------------------- |
| `clone`    | `fetch_add(1, AcqRel)` | `fetch_add(1, Relaxed)`          | Нет барьера на горячем пути clone |
| `drop`     | `fetch_sub(1, AcqRel)` | `fetch_sub(1, Release)`          | Нет Acquire на каждом drop        |
| Деструктор | —                      | `fence(Acquire)` при `prev == 1` | Acquire только на холодном пути   |

## Почему Relaxed для clone безопасен

```rust
fn clone(&self) -> Self {
    unsafe { &*self.inner }
        .ref_count
        .fetch_add(1, Ordering::Relaxed); // (1)
    Self { inner: self.inner }
}
```

| #   | Операция              | Memory order | Обоснование                                                                                                                                                                                                                                                                                                                       |
| --- | --------------------- | ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | `ref_count.fetch_add` | **Relaxed**  | Клонирующий поток уже владеет `SharedPtr` → `ref_count >= 1` → объект гарантированно жив. Мы не читаем данные через результат `fetch_add` и не зависим от значений, записанных другими потоками. Единственная гарантия, которая нам нужна - **modification order** (атомарность самого инкремента), и `Relaxed` её предоставляет. |

## Почему Release + fence(Acquire) вместо AcqRel

```rust
fn drop(&mut self) {
    let prev = unsafe { &*self.inner }
        .ref_count
        .fetch_sub(1, Ordering::Release); // (2)

    if prev == 1 {
        fence(Ordering::Acquire); // (3)
        unsafe { drop(Box::from_raw(self.inner)); } // (4)
    }
}
```

| #   | Операция                     | Memory order | Обоснование                                                                                                                                                                             |
| --- | ---------------------------- | ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 2   | `ref_count.fetch_sub`        | **Release**  | Публикует все записи этого потока. На горячем пути (`prev > 1`) Acquire не нужен - мы не читаем данные, только уменьшаем счётчик.                                                       |
| 3   | `fence`                      | **Acquire**  | Только при `prev == 1` (холодный путь). Fence синхронизируется с Release-операциями из release sequence `ref_count`, гарантируя видимость всех записей всех потоков перед деструктором. |
| 4   | `Box::from_raw` (деструктор) | -            | Безопасно: fence (3) установил happens-before.                                                                                                                                          |

## Release Sequence

![shared-ptr.png](../../assets/shared-ptr.png)

На диаграмме сценарий с тремя потоками:

- **T1** создаёт объект `Bar()`, передаёт указатель `p` в другой поток через **mo** (modification order)
- `p → Foo()`: второй поток обращается к данным через `deref`
- **`fa`** (`fetch_add`, 1 -> 2): clone с `Relaxed`, инкрементирует refcount
- **`fs`** (`fetch_sub`, 2 -> 1 и 1 -> 0): drop с `Release`, декрементирует refcount
- **`fa rlx`** (1 -> 2): промежуточная RMW-операция, часть release sequence
- Последний `fs` (1 -> 0) с `AcqRel` / `fence(Acquire)`: синхронизируется со всей **release sequence headed by `fs`**, вызывает деструктор

Ключевой момент: `synchronizes-with` работает через промежуточные потоки.

```
T1 (drop)            T2 (drop)            T3 (drop, последний)
──────────           ─────────            ────────────────────
fetch_sub(Release)
   1 -> 2
                     fetch_sub(Release)
                        2 -> 1
                                          fetch_sub(Release)
                                             1 -> 0
                                          fence(Acquire)
                                          <- видит ВСЕ записи T1 и T2
                                          drop(Box::from_raw)
```

Все `fetch_sub(Release)` - RMW-операции, образующие **release sequence** в modification order
`ref_count`. `fence(Acquire)` в T3 синхронизируется с Release-операциями всей цепочки.

## Выигрыш на слабых архитектурах

На x86 разница минимальна - все store уже имеют Release-семантику, а все load-Acquire.
Но на ARM/AArch64:

- `AcqRel` на RMW → полный барьер (`dmb ish`) **на каждом** clone/drop
- `Relaxed` на clone → **нет барьера** вообще
- `Release` на drop → половинный барьер (`dmb ishst`), дешевле полного
- `fence(Acquire)` при `prev == 1` → полный барьер, но **только один раз** за время жизни объекта

Это паттерн из [`std::sync::Arc`](https://doc.rust-lang.org/src/alloc/sync.rs.html) -
production-проверенная оптимизация.
