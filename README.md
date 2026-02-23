# rs_lib_ng

**rs_lib_ng** is a Rust library that provides utilities for configuration management, resilient HTTP retrieval, logging, and market data adapters (CNN, NASDAQ). The crate is organized into focused modules:

- **configs** — local and cloud configuration loading and merging.
- **retrieve** — resilient HTTP helper (`KyHttp`) with advanced retry semantics.
- **loggers** — lightweight async logging with worker and transport abstractions.
- **markets** — adapters for external market data (CNN, NASDAQ).
- **core** — shared error types and small core utilities.

**Quick start**

1. Add the crate to your workspace (path or registry).
2. Create a `Logger` with `loggers::builder::LoggerBuilder`.
3. Use `retrieve::ky_http::KyHttp` for all network calls to get consistent retry, backoff, and concurrency behavior.

**Repository quote (verbatim):**
> "Learning how to create a lib crate"

This README is a high-level entry point. See `docs/USAGE_KY_HTTP.md` for detailed examples showing how other modules should call `KyHttp`.
