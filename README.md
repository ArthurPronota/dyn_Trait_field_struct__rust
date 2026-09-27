# `dyn Trait` в полях структуры

## Что такое `dyn Trait`

**`dyn Trait`** — это **trait object** (trait-объект): **стирание типа** (type erasure) для **динамической диспетчеризации**. Позволяет хранить **разные** типы, реализующие **один** trait, в **одной** переменной или коллекции.

## Проблема: `dyn Trait` — **Unsized type**

**`dyn Trait`** — **DST** (Dynamically Sized Type). Размер **неизвестен** на этапе компиляции:

```rust
trait Logger {
    fn log(&self, msg: &str);
}

// ❌ Нельзя — размер неизвестен
struct App {
    logger: dyn Logger,   // ошибка
}
```

**Ошибка:**

```
error[E0277]: the size for values of type `dyn Logger` cannot be known at compilation time
```

**Причина:** `dyn Logger` может быть **любым** типом (`Console`, `FileLogger`, ...) — **размеры разные**.

## Решение: указатель

**`dyn Trait`** всегда **за** указателем. В поле структуры — **один из**:

| Указатель | Владение | Потоки | Когда |
|---|---|---|---|
| **`Box<dyn Trait>`** | ✅ Владеет | ⚠️ По `Send`/`Sync` | Один владелец |
| **`&dyn Trait`** | ❌ Заимствует | ⚠️ | Временное использование |
| **`Rc<dyn Trait>`** | ✅ Разделяет | ❌ | Один поток |
| **`Arc<dyn Trait>`** | ✅ Разделяет | ✅ | Много потоков |

## Разбор примера

```rust
trait Logger {
    fn log(&self, msg: &str);
}

struct App {
    logger: Box<dyn Logger + Send + Sync>,
}

struct Console;

impl Logger for Console {
    fn log(&self, msg: &str) {
        println!("{}", msg);
    }
}

fn main() {
    let app = App {
        logger: Box::new(Console),
    };

    app.logger.log("abc");   // abc
}
```

### Что происходит

1. **`trait Logger`** — контракт: `log(&self, msg: &str)`.
2. **`struct App`** — содержит `logger: Box<dyn Logger + Send + Sync>`.
3. **`Console`** — реализация `Logger`.
4. **`Box::new(Console)`** — **упаковывает** в кучу, создаёт **fat pointer**.
5. **`app.logger.log("abc")`** — **косвенный** вызов через vtable.

### Схема памяти

```
App:
+---------------------+
| logger: Box<dyn ...>|
|   ptr to data    ───┼──> [Console] (в куче)
|   ptr to vtable  ───┼──> [vtable]:
+---------------------+     +---------------+
                            | log() -> ...  |
                            +---------------+
```

## Почему `Box`

**`Box<dyn Trait>`** — **владеющий** указатель:

- **Владеет** объектом.
- **Освобождает** память при drop.
- **Размер** известен — `Box` — **thin pointer + vtable** (fat pointer, 16 байт).

### Альтернативы

#### `&dyn Trait` — заимствование

```rust
struct App<'a> {
    logger: &'a dyn Logger,
}

fn main() {
    let console = Console;
    let app = App { logger: &console };
    app.logger.log("abc");
}
```

**Плюс:** без кучи.
**Минус:** lifetime — `App` **не владеет** `logger`.

#### `Arc<dyn Trait>` — многопоточность

```rust
use std::sync::Arc;

struct App {
    logger: Arc<dyn Logger + Send + Sync>,
}

let logger = Arc::new(Console);
let app = App { logger: logger.clone() };
```

**Плюс:** разделяемое владение, `Send + Sync`.
**Минус:** атомарный счётчик — **дороже**.

#### `Rc<dyn Trait>` — один поток

```rust
use std::rc::Rc;

struct App {
    logger: Rc<dyn Logger>,
}
```

**Плюс:** дешевле `Arc`.
**Минус:** **не** `Send + Sync`.

## Почему `Send + Sync` в примере

```rust
struct App {
    logger: Box<dyn Logger + Send + Sync>,
}
```

**`Send + Sync`** — **дополнительные** trait bounds:

- **`Send`** — можно **перемещать** между потоками.
- **`Sync`** — можно **делить** `&` между потоками.

**Зачем:** `App` можно **передавать** в **другие потоки**:

```rust
let app = App { logger: Box::new(Console) };

std::thread::spawn(move || {
    app.logger.log("from thread");   // ✅ работает
});
```

**Без** `Send + Sync`:

```rust
struct App {
    logger: Box<dyn Logger>,   // ← нет Send + Sync
}

thread::spawn(move || {
    app.logger.log("...");   // ❌ ошибка: App не Send
});
```

**Ошибка:**

```
error[E0277]: `dyn Logger` cannot be sent between threads safely
```

## Почему `dyn Trait` — **fat pointer**

**`Box<dyn Trait>`** — **два** слова (16 байт):

1. **Указатель** на данные.
2. **Указатель** на vtable.

**`Box<T>`** для **конкретного** типа — **одно** слово (8 байт).

```rust
use std::mem::size_of;

println!("{}", size_of::<Box<Console>>());       // 8
println!("{}", size_of::<Box<dyn Logger>>());    // 16
```

## Внедрение зависимостей (Dependency Injection)

**Классический паттерн** — `App` **не знает** конкретный `Logger`:

```rust
struct App {
    logger: Box<dyn Logger>,
}

impl App {
    fn new(logger: Box<dyn Logger>) -> Self {
        App { logger }
    }

    fn run(&self) {
        self.logger.log("App started");
    }
}

fn main() {
    // Продакшн
    let app = App::new(Box::new(Console));
    app.run();

    // Тесты — мок
    struct MockLogger;
    impl Logger for MockLogger {
        fn log(&self, _msg: &str) { /* ничего */ }
    }

    let test_app = App::new(Box::new(MockLogger));
    test_app.run();
}
```

**Плюсы:**

- **Гибкость** — любой `Logger`.
- **Тестируемость** — легко **подменить**.
- **Слабая связанность**.

## Когда **владеть**, когда **заимствовать**

| Ситуация | Решение |
|---|---|
| **Структура владеет** объектом | `Box<dyn Trait>` |
| **Структура долго живёт**, владеет | `Box` или `Arc` |
| **Структура использует** чужой объект | `&dyn Trait` |
| **Много потоков** | `Arc<dyn Trait + Send + Sync>` |
| **Один поток**, разделение | `Rc<dyn Trait>` |

## Сводная таблица

| Указатель | Владение | Send/Sync | Размер | Когда |
|---|---|---|---|---|
| **`Box<dyn T>`** | ✅ Владеет | ⚠️ | 16 байт | Один владелец |
| **`&dyn T`** | ❌ Заимствует | ⚠️ | 16 байт | Временное |
| **`&mut dyn T`** | ❌ Заимствует | ❌ | 16 байт | Изменение |
| **`Rc<dyn T>`** | ✅ Разделяет | ❌ | 16 байт | Один поток |
| **`Arc<dyn T>`** | ✅ Разделяет | ✅ | 16 байт | Много потоков |

## Полный пример: DI с `Arc`

```rust
use std::sync::Arc;

trait Logger: Send + Sync {
    fn log(&self, msg: &str);
}

struct Console;
impl Logger for Console {
    fn log(&self, msg: &str) {
        println!("[Console] {}", msg);
    }
}

struct FileLogger;
impl Logger for FileLogger {
    fn log(&self, msg: &str) {
        println!("[File] {}", msg);
    }
}

struct App {
    logger: Arc<dyn Logger>,
}

impl App {
    fn new(logger: Arc<dyn Logger>) -> Self {
        App { logger }
    }

    fn run(&self) {
        self.logger.log("App started");
    }
}

fn main() {
    // Продакшн
    let console = Arc::new(Console);
    let app = App::new(console);
    app.run();

    // Другой логгер
    let file = Arc::new(FileLogger);
    let app2 = App::new(file);
    app2.run();

    // Многопоточность
    let app3 = App::new(Arc::new(Console));
    std::thread::spawn(move || {
        app3.run();
    }).join().unwrap();
}
```

## Сводная таблица

| Аспект | Описание |
|---|---|
| **`dyn Trait`** | Trait object |
| **Размер** | **Неизвестен** (DST) |
| **В поле** | **Только** за указателем |
| **Указатели** | `Box`, `&`, `Rc`, `Arc` |
| **`Box`** | Владеет |
| **`&`** | Заимствует |
| **`Arc`** | Разделяет + потоки |
| **`Send + Sync`** | Для многопоточности |
| **Размер указателя** | 16 байт (fat pointer) |
| **Паттерн** | **Dependency Injection** |

## Итог

- **`dyn Trait`** — **trait object** для **динамической** диспетчеризации.
- **`dyn Trait`** — **DST** (размер **неизвестен** на этапе компиляции).
- В **поле структуры** — **только** за **указателем**.
- **Указатели:**
  - **`Box<dyn Trait>`** — **владеет**;
  - **`&dyn Trait`** — **заимствует** (с lifetime);
  - **`Rc<dyn Trait>`** — **разделяет** (один поток);
  - **`Arc<dyn Trait>`** — **разделяет** (много потоков).
- **`Send + Sync`** — если нужно **передавать** между потоками.
- **Fat pointer** — 16 байт (данные + vtable).
- **Паттерн** — **Dependency Injection**: `App` не знает **конкретный** `Logger`.
- **Правило:** владеет → `Box`/`Arc`; использует → `&dyn Trait`; многопоточность → `Arc + Send + Sync`.
