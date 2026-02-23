# Logger — Reference and Usage

**Purpose:** The crate's logger subsystem provides a small, async-friendly logging abstraction with a **builder**, **worker**, and **transport** layers so library code and applications can emit structured logs consistently and without blocking.

**Repository quotes (verbatim):**
> "Learning how to create a lib crate"

> "✅ Module logic bypass: Retry increased to 10 to ensure success window is hit."

---

## Overview

The logger is intentionally lightweight and designed for library use:

- **Non-blocking**: log emission is handed off to a background worker so callers don't block on I/O.
- **Structured**: logs include component name, level, timestamp, and optional structured fields.
- **Pluggable transports**: you can register multiple transports (console, file, remote) that implement a simple transport trait.
- **Cloneable**: the `Logger` handle is cheap to clone and share across tasks and threads.

Use the `LoggerBuilder` to construct a `Logger` instance, then pass clones into modules (for example, `KyHttp`, market adapters, or config loaders) so all components share consistent logging behavior.

---

## Key concepts and types

- **`LoggerBuilder`** — fluent builder used to configure component name, default level, and transports.
- **`Logger`** — the handle used by application code. Lightweight and cloneable.
- **`LogLevel`** — enum for `Trace`, `Debug`, `Info`, `Warn`, `Error`, `Fatal`.
- **`Transport`** — trait that concrete transports implement (e.g., `ConsoleTransport`, `FileTransport`, `HttpTransport`).
- **`Worker`** — background task that receives log events and dispatches them to registered transports.
- **`LogRecord`** — internal struct representing a single log event (timestamp, level, message, fields, component).

**Behavioral notes**
- The worker uses an async channel with bounded capacity to avoid unbounded memory growth.
- If the channel is full, the logger will drop low-priority messages (configurable) and emit a single warning to the worker to avoid log storms.
- Transports are executed in the worker context; long-running transport operations should be implemented asynchronously to avoid blocking other log dispatches.

---

## API (common functions and signatures)

> The exact function names and signatures may vary slightly depending on the crate version; the examples below reflect the public surface used across the codebase.

### Builder and logger creation
```rust
use rs_lib_ng::loggers::builder::LoggerBuilder;

// Build a logger for component "my-service"
let logger = LoggerBuilder::new("my-service")
    .level(rs_lib_ng::loggers::core::LogLevel::Info)
    .with_console()          // convenience to add a console transport
    .with_file("logs/app.log") // optional file transport
    .build()?;               // returns Result<Logger, NgError>
```
### Emitting logs
```Rust
// The Logger implements convenience methods and macros
logger.info("Service started");
logger.debug_with_fields("Received request", &[("path", "/v1/data"), ("id", "abc123")]);
logger.error("Failed to parse response");
```
### Using macros (if provided)
```Rust
// If the crate exposes macros that accept a logger handle:
rs_lib_ng::loggers::info!(logger, "Connected to {}", host);
rs_lib_ng::loggers::warn!(logger, "Retrying after {}s", retry_delay);
```
### Transport trait (example)
```Rust
#[async_trait::async_trait]
pub trait Transport: Send + Sync + 'static {
    async fn send(&self, record: LogRecord) -> Result<(), TransportError>;
}
```
### Registering a custom transport
```Rust
use rs_lib_ng::loggers::transports::Transport;
use rs_lib_ng::loggers::builder::LoggerBuilder;

struct MyTransport { /* config */ }

#[async_trait::async_trait]
impl Transport for MyTransport {
    async fn send(&self, record: LogRecord) -> Result<(), TransportError> {
        // send to remote endpoint or write to file
        Ok(())
    }
}

let logger = LoggerBuilder::new("component")
    .with_transport(Box::new(MyTransport { /* ... */ }))
    .build()?;
```

## Usage examples
### Minimal console logger
```Rust
use rs_lib_ng::loggers::builder::LoggerBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let logger = LoggerBuilder::new("minimal")
        .with_console()
        .build()?;

    logger.info("Application starting");
    Ok(())
}
```
### Async service wiring (recommended)
```Rust
use rs_lib_ng::loggers::builder::LoggerBuilder;
use rs_lib_ng::retrieve::ky_http::KyHttp;
use tokio::task;

#[tokio::main]
async fn main() -> Result<(), rs_lib_ng::core::error::NgError> {
    // Create logger and share it across components
    let logger = LoggerBuilder::new("my-service")
        .level(rs_lib_ng::loggers::core::LogLevel::Debug)
        .with_console()
        .build()?;

    // Create KyHttp using the same logger
    let ky = KyHttp::new(logger.clone());

    // Spawn background worker tasks that also use the logger
    let jh = task::spawn(async move {
        logger.info("Background worker started");
        // do work...
    });

    // Use ky for HTTP calls; it will log via the same logger instance
    let _ = ky.get::<serde_json::Value>("https://api.example.com/health", reqwest::header::HeaderMap::new()).await?;

    jh.await?;
    Ok(())
}
```
### Custom transport with structured fields
```Rust
use rs_lib_ng::loggers::{builder::LoggerBuilder, core::LogRecord};

struct JsonHttpTransport { endpoint: String }

#[async_trait::async_trait]
impl rs_lib_ng::loggers::transports::Transport for JsonHttpTransport {
    async fn send(&self, record: LogRecord) -> Result<(), rs_lib_ng::loggers::transports::TransportError> {
        // Serialize record to JSON and POST to endpoint
        let body = serde_json::to_vec(&record)?;
        let client = reqwest::Client::new();
        client.post(&self.endpoint).body(body).send().await.map_err(|e| /* map error */)?;
        Ok(())
    }
}

let logger = LoggerBuilder::new("http-logger")
    .with_transport(Box::new(JsonHttpTransport { endpoint: "https://logs.example.com/ingest".into() }))
    .build()?;
```

### Configuration and tuning
Log level — set via LoggerBuilder::level(...). Use Info or Warn in production; Debug for development.

Channel capacity — the worker channel has a bounded capacity; increase it if your application emits bursts of logs.

Drop policy — configure whether low-priority messages are dropped when the channel is full, and whether a single "dropped logs" counter is emitted.

Flush semantics — the worker exposes a flush() method to synchronously wait for pending logs to be delivered (useful in shutdown paths).

### Example: graceful shutdown
```Rust
// On shutdown
logger.info("Shutting down");
logger.flush().await?; // wait for worker to finish sending logs
```
### Extensibility and best practices
Keep transports async — avoid blocking operations inside send to prevent delaying other log dispatches.

Avoid heavy serialization on hot paths — if you need expensive formatting, consider deferring it to the transport or using a lightweight pre-serialized field.

Share a single logger instance — clone the Logger handle rather than creating multiple independent workers; this centralizes transport management and reduces resource usage.

Use structured fields — prefer logger.info_with_fields("msg", &[("key","value")]) so transports can index and filter log

### Example test transport (in-memory)
```Rust
use rs_lib_ng::loggers::transports::Transport;
use std::sync::{Arc, Mutex};

struct TestTransport { records: Arc<Mutex<Vec<LogRecord>>> }

#[async_trait::async_trait]
impl Transport for TestTransport {
    async fn send(&self, record: LogRecord) -> Result<(), TransportError> {
        self.records.lock().unwrap().push(record);
        Ok(())
    }
}
```

### Final notes
The logger is designed to be safe for library use:
it avoids global state and encourages passing a Logger handle into modules.

For production systems, combine console/file transports with a remote transport that supports batching and backpressure.
