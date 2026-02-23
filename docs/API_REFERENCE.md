
---

### docs/API_REFERENCE.md
```markdown
# API Reference (high level)

This file summarizes the public modules and the most important types you will interact with.

## Core
- **`NgError`** — central error enum used across the crate.

## Configs
- **`ConfigManager`**
  - `get_local_config(path: &str) -> Result<Self, NgError>` — loads local JSON/TOML and merges `WEBLIB_` env vars.
  - `get_cloud_config(url: &str) -> Result<Self, NgError>` — downloads and decrypts remote config (uses `configs::cloud::load_remote_json`).
  - `get(&self) -> Arc<Value>` — returns the current config snapshot.

## Retrieve
- **`KyOptions`** — options for `KyHttp`.
- **`KyHttp`** — main HTTP helper. Methods:
  - `get<T>(&self, url: &str, headers: HeaderMap) -> Result<ApiResponse<T>, NgError>`
  - `post<T, B>(&self, url: &str, headers: HeaderMap, body: &B) -> Result<ApiResponse<T>, NgError>`
  - `put`, `patch`, `delete`, `head`, `options`, `trace` — similar signatures.

## Loggers
- **`LoggerBuilder`** — construct `Logger` with component name and level.
- **Logging macros** — `trace!`, `debug!`, `info!`, `warn!`, `error!`, `fatal!` — use the `Logger` instance.

## Markets
- **CNN** — `CnnApi`, `FearAndGreed` helpers.
- **NASDAQ** — `NasdaqApi`, `MarketStatus`, `YahooStreaming` (WebSocket streaming).

**Note:** All network-facing modules use `KyHttp` for HTTP calls (see `markets::cnn::apicallcnn` and `markets::nasdaq::apicallnasdaq`).
