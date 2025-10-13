# API Reference

## OAuth Endpoints

### `GET /oauth/authorize`

Query Parameters:

- `user_id` (required) — Stable identifier provided by the MCP client.
- `redirect_uri` (optional) — Overrides the configured redirect URI for multi-host setups.

Response:

```json
{
  "authorize_url": "https://accounts.google.com/o/oauth2/v2/auth?...",
  "csrf_state": "STATE",
  "pkce_verifier": "VERIFIER"
}
```

Persist the `csrf_state` and `pkce_verifier` client-side to complete the OAuth flow.

### `GET /oauth/callback`

Query Parameters supplied by Google:

- `state`
- `code`

Response: `{ "status": "authorized" }` on success. Errors return HTTP 4xx/5xx with a JSON body `{ "error": "..." }`.

## Remote MCP Transport (SSE)

The server exposes Model Context Protocol over Server-Sent Events using the official `rmcp` SSE transport.

- **SSE stream:** `GET /mcp/sse`
  - Responds with an SSE stream. The first event contains the message posting URL (`/mcp/message`).
  - Clients should keep the stream open and send JSON-RPC payloads to the POST endpoint indicated in the first event.
- **JSON-RPC ingress:** `POST /mcp/message?sessionId=<id>`
  - Accepts JSON-RPC messages produced by the MCP client. `sessionId` is provided in the initial SSE event.

Both endpoints live under the same base URL as the OAuth routes. Each SSE session spins up a fresh `CalendarService` backed by the RMCP `ServerHandler` implementation.

## MCP Tool Endpoint

### `POST /mcp/tool`

All requests share the envelope:

```json
{
  "operation": "list|get|create|update",
  "user_id": "example-user",
  "params": { ... },
  "payload": { ... }
}
```

If the user is not authorized the response is:

```json
{
  "status": "UNAUTHORIZED",
  "data": {
    "authorize_url": "...",
    "state": "STATE",
    "pkce_verifier": "VERIFIER"
  },
  "error": "authorization required"
}
```

### Operations

- **list** — Accepts `params` matching `ListEventsParams` (time range, pagination options). Returns the raw Google response mapped into `ListEventsResponse`.
- **get** — Requires `event_id`. Optional `calendar_id` overrides the default calendar.
- **create** — Body uses `EventPayload` (summary, start/end, attendees, reminders). `summary`, `start`, and `end` must be provided.
- **update** — Same payload as create plus the `event_id` field in the envelope. Performs a PATCH request, preserving unspecified fields.

All successful responses use `{ "status": "SUCCESS", "data": <payload> }`.
