# oii-check

`oii-check` is a proctoring and network restriction monitoring wrapper designed for competitive programming contests
(such as the Italian Olympiad in Informatics, Terry, and CMS).

It embeds the target contest interface inside a full-window iframe while running client-side verification tasks
(internet restriction checks and periodic screen captures) that report back to a backend server.

---

## Features

- **Internet Restriction Verification**:
  - Periodically attempts to fetch predefined external endpoints (`TESTS`) to verify that internet access is blocked or
    restricted according to contest rules.
  - Can be toggled on or off via the client configuration flag `ENABLE_INTERNET_CHECK`.
  - Reports reachability results to `POST /internet`.

- **Screen Capture & Proctoring**:
  - Prompts contestants to share their entire monitor screen using `navigator.mediaDevices.getDisplayMedia`.
  - Captures frames at configurable intervals, scales them to bounds (`SCREEN_CAPTURE_MAX_WIDTH` /
    `SCREEN_CAPTURE_MAX_HEIGHT`), dynamically selects JPEG or PNG encoding based on file size, and posts them to
    `POST /screen`.
  - Can be toggled on or off via the client configuration flag `ENABLE_SCREEN_RECORDING`.

- **Client-Side Queueing & Fault Tolerance**:
  - Buffers telemetry payloads in memory if the connection drops.
  - Automatically flushes queued requests once connectivity is restored.
  - Prunes expired requests older than `SEND_BUFFER_MAX_MINUTES` to avoid memory bloat.

- **Device Fingerprinting**:
  - Generates and stores a unique 128-bit hex client fingerprint in `localStorage` (`CLIENT_ID_STORAGE_KEY`) to track
    submissions across page reloads.

- **Contestant Navigation Warning**:
  - Registers a `beforeunload` listener to warn contestants if they attempt to close or navigate away from the
    proctoring window during an active contest.

- **Flexible Authentication Backend**:
  - **CMS Cookie Authentication**: Validates signed Tornado 4 session cookies (HMAC-SHA256) used by CMS.
  - **HTTP Header Authentication**: Extracts the user identifier from a configured custom header (e.g. `X-OII-AUTH`).

---

## Configuration

The backend is configured via a TOML file (defaulting to `config.toml`, or passed as the first command-line argument).
An example template is provided in [`config.example.toml`](config.example.toml).

Client-side parameters are configured in [`static/js/config.js`](static/js/config.js).

### Server Options

| Parameter | Type | Description |
|-----------|------|-------------|
| `listen` | string | Socket address to bind to (e.g. `"127.0.0.1:3000"` or `"0.0.0.0:8080"`). |
| `data_dir` | string | Directory where telemetry logs and captured screenshots are stored. |
| `auth.method` | string | Authentication method (`"cms_cookie"` or `"header"`). |
| `auth.cookie_name` | string | Cookie name when using CMS cookie authentication. |
| `auth.secret` | string | Shared HMAC secret used to verify CMS signed cookies (hex-encoded or raw string). |
| `auth.header_name` | string | Name of the HTTP request header when using header authentication. |

---

## Storage & Data Format

Collected data is organized per user in the configured `data_dir`:

```text
data/
└── <username_or_token>/
    ├── logs.json
    └── <fingerprint>/
        ├── 1775836491.512.jpg
        └── 1775836501.530.png
```

### `logs.json` Format

Each line is a JSON object corresponding to either an internet check or a screenshot event.

#### Internet Check Record

```json
{
  "client_ts": 1775836490.123,
  "fp": "c8d0a5183f3e9c4b7260e1d0f85b34a1",
  "ic": [true, true, true, true],
  "server_ts": 1775836490.155
}
```

- `client_ts`: Unix timestamp (in seconds) recorded on the client device.
- `server_ts`: Unix timestamp (in seconds) recorded when the server received the payload.
- `fp`: 32-character hexadecimal client fingerprint.
- `ic`: Array of booleans representing test outcomes (`true` indicates access was successfully blocked/restricted).

#### Screen Capture Record

```json
{
  "client_ts": 1775836491.512,
  "fp": "c8d0a5183f3e9c4b7260e1d0f85b34a1",
  "filename": "1775836491.512.jpg",
  "server_ts": 1775836491.545
}
```

- `filename`: Image filename within `{data_dir}/{user}/{fp}/`.

---

## Building and Running

### Prerequisites

- [Rust](https://www.rust-lang.org/) (2024 edition, Rust 1.85+)

### Build

```bash
cargo build --release
```

### Run

1. Copy and adjust the configuration:
   ```bash
   cp config.example.toml config.toml
   ```

2. Start the service:
   ```bash
   ./target/release/oii-check config.toml
   ```

   Or run directly via Cargo:
   ```bash
   cargo run --release -- config.toml
   ```
