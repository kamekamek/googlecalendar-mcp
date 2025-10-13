# Google Calendar MCP Server — Design Notes

## Overview

The service exposes a Model Context Protocol (MCP) bridge that allows an agent client to initiate OAuth authorization with Google, store refreshable access tokens, and perform non-destructive Google Calendar operations (`list`, `get`, `create`, `update`). Event deletion is deliberately omitted to avoid accidental data loss.

## Architecture

- **HTTP façade (Axum 0.8)** — Serves OAuth endpoints (`/oauth/authorize`, `/oauth/callback`) and a JSON MCP tooling endpoint (`/mcp/tool`).
- **Remote MCP transport (rmcp::transport::sse_server)** — Provides a fully compliant SSE gateway mounted under `/mcp/sse` & `/mcp/message`, sharing the same Axum instance. Each SSE connection spins up a fresh `CalendarService` based on the official RMCP `ServerHandler` implementation.
- **OAuth module (`src/oauth`)** — Wraps the `oauth2` crate to produce PKCE challenges, exchange authorization codes, refresh tokens, and serialize token state.
- **Token storage (`FileTokenStorage`)** — Persists tokens to disk, with an internal cache guarded by `RwLock`. The implementation abstracts persistence to allow future hardware-backed storage.
- **Token storage (`InMemoryTokenStorage`)** — Optional purely in-memory backend for short-lived sessions or test environments.
- **Google Calendar client (`src/google_calendar`)** — Thin wrapper over `reqwest` with strongly typed payloads for Google Calendar API operations. Handles RFC3339 timestamp formatting.
- **MCP server (`src/mcp`)** — Exposes RMCP `ServerHandler` + `#[tool]` annotated methods, translating tool requests into Google API calls, enforcing token freshness, and shaping structured responses. Also retains a lightweight HTTP shim for `/mcp/tool` requests.

## Request Flow

1. Agent calls `/mcp/tool`.
2. If no token exists, the server responds `401` with an authorization URL (PKCE+state stored in-memory).
3. User completes consent flow; Google redirects to `/oauth/callback`.
4. The callback exchanges the authorization code for tokens, persists them, and the agent can resume tool calls.

## Security Considerations

- PKCE verifier/state pairs are stored in-memory for 10 minutes and removed after use.
- Tokens are cached in-memory and mirrored to `config/tokens.json`. The `encrypt_tokens` flag is plumbed for future enhancements.
- Logging avoids printing token values. Sensitive configuration is supplied via env/config files, never hard-coded.

## Future Enhancements

- Replace file-based token storage with a secrets manager or OS keychain integration.
- Implement token encryption using platform-specific APIs.
- Emit structured telemetry (OpenTelemetry) and metrics for observability.
- Add synthetic integration tests hitting Google's sandbox (requires recorded fixtures).
- Support SSE streaming transports beyond HTTP (e.g., WebTransport) as the RMCP SDK evolves.
