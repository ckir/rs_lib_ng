# KyHttp Usage Guide

**Purpose:** `KyHttp` is a resilient HTTP helper intended for other modules to perform HTTP requests with robust retry/backoff, concurrency limiting, and deterministic test hooks.

---

## Key types and responsibilities

- **`KyOptions`** — configuration for timeouts, retry counts, allowed methods, backoff limits, concurrency limits, and test-mode flags.
- **`KyHttp`** — the client wrapper. Construct with `KyHttp::new(logger)` or `KyHttp::new_with_opts(logger, Some(opts))`.
- **`ApiResponse<T>`** — returned by `get`, `post`, `put`, `patch`, `delete`, `head`, `options`, `trace`. Contains `data: Option<T>`, `status`, `success`, `headers`, `error_body`.

---

## Creating a logger and client

```rust
use rs_lib_ng::loggers::builder::LoggerBuilder;
use rs_lib_ng::retrieve::ky_http::{KyHttp, KyOptions};
use std::time::Duration;
use reqwest::header::HeaderMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Build a logger (used by KyHttp for structured logs)
    let logger = LoggerBuilder::new("my-component").build()?;

    // Default client
    let client = KyHttp::new(logger.clone());

    // Custom options
    let mut opts = KyOptions::default();
    opts.retry = 3;
    opts.timeout = Duration::from_secs(10);
    opts.backoff_limit = Some(Duration::from_secs(2));
    opts.test_mode = false;

    let client_with_opts = KyHttp::new_with_opts(logger, Some(opts));

    // Use client_with_opts for requests...
    Ok(())
}
```
## Performing requests
KyHttp exposes convenience async methods. Each returns Result<ApiResponse<T>, NgError>

### GET example
```Rust
use reqwest::header::HeaderMap;
use serde::Deserialize;

#[derive(Deserialize)]
struct MyResponse { pub value: String }

let headers = HeaderMap::new();
let res = client.get::<MyResponse>(&"https://api.example.com/data", headers).await?;

if res.success {
    let data = res.data.unwrap();
    println!("value = {}", data.value);
} else {
    eprintln!("Request failed: status {}", res.status);
}
```

### GET example (typed response)
```Rust
use serde::Deserialize;
use reqwest::header::HeaderMap;
use rs_lib_ng::retrieve::ky_http::KyHttp;

#[derive(Deserialize)]
struct MyResponse { pub value: String }

async fn example_get(client: &KyHttp) -> Result<(), rs_lib_ng::core::error::NgError> {
    let headers = HeaderMap::new();
    let res = client.get::<MyResponse>("https://api.example.com/data", headers).await?;

    if res.success {
        let data = res.data.expect("expected data on success");
        println!("value = {}", data.value);
    } else {
        eprintln!("Request failed: status {}", res.status);
    }
    Ok(())
}
```

### POST example (JSON body, generic response)
```Rust
use serde::Serialize;
use reqwest::header::HeaderMap;
use rs_lib_ng::retrieve::ky_http::KyHttp;

#[derive(Serialize)]
struct Payload { pub name: String }

async fn example_post(client: &KyHttp) -> Result<(), rs_lib_ng::core::error::NgError> {
    let payload = Payload { name: "alice".into() };
    let headers = HeaderMap::new();

    // When the response shape is unknown, use serde_json::Value
    let res = client.post::<serde_json::Value, _>("https://api.example.com/create", headers, &payload).await?;

    if res.success {
        println!("Created: {}", res.status);
    } else {
        eprintln!("Create failed: {} - {:?}", res.status, res.error_body);
    }
    Ok(())
}
```
## Overriding Request Options
You can pass `KyOptions` to any KyHttp call to change retries or timeouts for that specific request without changing global settings.

```rust
let mut opts = KyOptions::default();
opts.retry = 5; // Increase retries for unstable connections
let status = service.fetch_status(Some(opts)).await?;
```
