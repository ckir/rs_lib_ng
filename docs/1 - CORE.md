## Core

### `NgError`
Central error registry for the library.

- **`NonJsonResponse`**: Triggered when Nasdaq returns HTML (Maintenance/Bot Challenge).
- **`NasdaqBusinessError`**: Triggered when `rCode` in the JSON is not 200.
- **`MalformedResponse`**: Triggered when mandatory fields or dates fail to parse.
