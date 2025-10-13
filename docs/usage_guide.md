# Usage Guide

## Prerequisites

1. Install the Rust nightly toolchain (`rustup toolchain install nightly`) — required because `rmcp` targets the upcoming 2024 edition. A `rust-toolchain.toml` file is provided, so running `cargo` inside this repository automatically selects nightly.
2. Create a Google Cloud OAuth client (web application type) with the redirect URI `http://localhost:8080/oauth/callback`.
3. Copy `.env.example` (at the repository root) to `.env` and populate `APP__OAUTH__CLIENT_ID` and `APP__OAUTH__CLIENT_SECRET`. Set `APP__SECURITY__USE_IN_MEMORY=true` if you prefer not to persist tokens to disk.
4. Optionally adjust `config/config.toml` for custom bind addresses, default calendar IDs, or storage mode.

## Running the Server

```bash
cargo +nightly run
```

The server listens on the configured `bind_address` (default `127.0.0.1:8080`).

## Authorizing a User

1. Direct the agent to call `GET /oauth/authorize?user_id=<agent-user-id>`.
2. Visit the returned `authorize_url` in a browser and sign in.
3. After granting consent, Google redirects to `/oauth/callback`; the server persists tokens for the specified `user_id`.

Tokens are stored at `config/tokens.json`. Secure this path appropriately.

## Invoking MCP Tools

- Remote MCP: point an MCP client (Claude Desktop, Cursor, etc.) at `http://localhost:8080/mcp/sse`. The first SSE event returns the POST target (`/mcp/message?sessionId=...`).
- HTTP shim: `POST /mcp/tool` with the desired operation.
- Always include the `user_id` in tool calls so the server can look up the correct token. Handle `UNAUTHORIZED` responses by prompting the human to complete OAuth again.

### Example — List Events

```json
{
  "operation": "list",
  "user_id": "demo-user",
  "params": {
    "time_min": "2025-10-13T00:00:00Z",
    "time_max": "2025-10-20T00:00:00Z",
    "single_events": true,
    "order_by_start_time": true
  }
}
```

## Testing

- `cargo test` executes unit tests for configuration, payload serialization, and token storage.
- Mock the Google API by overriding `google.api_base` to point at a local server during integration testing.
