## Markets: Nasdaq

### `NasdaqApi`
The resilient HTTP client for Nasdaq communications.

#### Methods
- **`async call(endpoint: &str, options: Option<KyOptions>) -> Result<Value, NgError>`** Executes a validated request with browser-mimicry headers and `rCode` checking.

**Note:** All network-facing modules use `KyHttp` for HTTP calls (see `markets::cnn::apicallcnn` and `markets::nasdaq::apicallnasdaq`).


### `MarketStatus`
High-level service for coordinating market-aware execution.

#### Methods
- **`new(logger: Logger) -> Self`** Initializes the service with a shared logger.
- **`async fetch_status(options: Option<KyOptions>) -> Result<MarketStatusData, NgError>`** Retrieves and deserializes the current market status from Nasdaq.
- **`is_regular_session(status: &MarketStatusData) -> bool`** Returns true if the current Eastern Time is within regular hours (09:30 - 16:00) on a business day.
- **`get_next_opening_delay(status: &MarketStatusData) -> Result<Duration, NgError>`** Calculates the precise time remaining until the next market open. Returns an error if the API date is malformed.
- **`async wait_until_open(status: &MarketStatusData)`** Asynchronously blocks until the next market opening time.
# Basic Nasdaq Request
To perform a simple raw call to a Nasdaq endpoint:

```rust
let api = NasdaqApi::new(logger);
let response = api.call("[https://api.nasdaq.com/api/market-info/](https://api.nasdaq.com/api/market-info/)", None).await?;