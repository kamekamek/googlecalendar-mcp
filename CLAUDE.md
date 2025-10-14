# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Google Calendar MCP (Model Context Protocol) server written in Rust that bridges MCP clients (like Claude Code, Cursor) to Google Calendar API. It provides OAuth-authenticated access to calendar operations (list, get, create, update) while intentionally omitting delete operations to prevent accidental data loss.

## Essential Commands

### Build and Run
```bash
# Build the project
cargo +nightly build

# Run the server
cargo +nightly run

# Run tests
cargo +nightly test
```

**Important:** This project requires **Rust nightly** due to the `rmcp` crate's Edition 2024 requirement. The `rust-toolchain.toml` file ensures nightly is automatically selected.

## Architecture

### Core Components

1. **HTTP Server (Axum 0.8)** - `src/main.rs`, `src/handlers/mod.rs`
   - OAuth endpoints: `/oauth/authorize`, `/oauth/callback`
   - Remote MCP transport: `/mcp` (SSE stream), `/mcp/message` (JSON-RPC ingress)
   - Legacy HTTP tool endpoint: `/mcp/tool` (for testing)
   - Optional OAuth proxy endpoints: `/proxy/oauth/*` (when `proxy.enabled = true`)

2. **MCP Service Layer** - `src/mcp/mod.rs`
   - `CalendarService`: Implements `rmcp::ServerHandler` with `#[tool]` annotations
   - Provides four tools: `google_calendar_list_events`, `get_event`, `create_event`, `update_event`
   - `ensure_token()`: Core authentication logic that handles token fetching, expiry checking, and automatic refresh
   - `HttpMcpServer`: Legacy HTTP-based wrapper for testing without full MCP client

3. **OAuth Management** - `src/oauth/mod.rs`, `src/oauth/storage.rs`
   - `OAuthClient`: Handles PKCE flow, authorization URL generation, code exchange, token refresh
   - `TokenStorage` trait with two implementations:
     - `FileTokenStorage`: Persists to `config/tokens.json` (default)
     - `InMemoryTokenStorage`: Ephemeral storage for testing/demos
   - `AuthorizationSession`: Short-lived (10 min) PKCE state/verifier cache

4. **Google Calendar Client** - `src/google_calendar/mod.rs`
   - Thin `reqwest`-based wrapper around Google Calendar API v3
   - `EventDateTime`: Custom deserializer that accepts both RFC3339 strings and object format
   - All requests use bearer token authentication

5. **Bearer Token Ingestion** - `src/token_ingest.rs`
   - Extracts `Authorization: Bearer <token>` headers from incoming requests
   - Allows OAuth proxy to forward tokens obtained via Dynamic Client Registration
   - Used when `proxy.enabled = true` to support Claude Code's OAuth 2.1 requirements

6. **OAuth Proxy (Optional)** - `src/proxy/mod.rs`
   - Implements Dynamic Client Registration endpoints required by Claude Code
   - Routes: `/.well-known/oauth-authorization-server`, `/proxy/oauth/register`, `/proxy/oauth/authorize`, `/proxy/oauth/token`, `/proxy/oauth/callback`
   - Bridges between MCP client's DCR expectations and Google's fixed OAuth client

7. **Configuration** - `src/config/mod.rs`
   - Hierarchical loading: `config/config.toml` → `config/config.local.toml` → `APP__*` environment variables
   - Key settings: `server.bind_address`, `oauth.client_id/client_secret`, `google.api_base`, `security.use_in_memory`, `proxy.enabled`

### Request Flow

**Standard OAuth Flow:**
1. MCP client calls tool → `401` with authorize URL (if not authenticated)
2. User visits authorize URL → Google OAuth consent
3. Google redirects to `/oauth/callback` → server stores token
4. Subsequent tool calls succeed using stored token

**With OAuth Proxy (for Claude Code):**
1. Claude Code discovers DCR metadata at `/.well-known/oauth-authorization-server`
2. Registers dynamic client via `/proxy/oauth/register`
3. OAuth flow proceeds through proxy endpoints → Google OAuth
4. Proxy injects bearer token in `Authorization` header → ingested by `token_ingest.rs`

### Key Design Patterns

- **Token Lifecycle**: All tools call `ensure_token()` which fetches → checks expiry → refreshes if needed → persists
- **User Isolation**: Every request requires `user_id` parameter; tokens are stored per-user
- **Error Handling**: All Google API errors are wrapped in `ErrorData::internal_error`; missing tokens return `ErrorData::invalid_request`
- **Transport Abstraction**: SSE transport (via `rmcp::transport::sse_server`) and HTTP tool endpoint share the same `CalendarService` implementation

## Configuration Setup

1. Copy `.env.example` → `.env`
2. Set required variables:
   ```env
   APP__OAUTH__CLIENT_ID="<from Google Cloud Console>"
   APP__OAUTH__CLIENT_SECRET="<from Google Cloud Console>"
   APP__OAUTH__REDIRECT_URI="http://localhost:8080/oauth/callback"
   ```
3. In Google Cloud Console:
   - Enable Google Calendar API
   - Create OAuth 2.0 Web Application credentials
   - Add redirect URI: `http://localhost:8080/oauth/callback`
   - For proxy mode, also add: `http://localhost:8080/proxy/oauth/callback`

## Testing Strategy

- **Unit tests**: Configuration defaults, serialization, token storage mechanics
- **Integration tests**: Not yet implemented (would require Google API mocks or VCR-style recording)
- **Manual testing**: Use `POST /mcp/tool` endpoint with JSON payloads before connecting MCP clients

## Common Pitfalls

1. **Edition 2024 requirement**: Always use `cargo +nightly` commands
2. **Token expiry**: Refresh tokens are only issued on first authorization; offline access must be requested in OAuth scopes
3. **EventDateTime format**: Accepts RFC3339 strings (`"2025-10-15T06:00:00+09:00"`) or objects (`{"dateTime": "...", "time_zone": "..."}`); JSON escaping in parameters can cause parse errors
4. **Proxy mode**: Requires `proxy.enabled = true` AND correct redirect URI in Google Console AND HTTPS termination (via Caddy/Nginx/mkcert)
5. **State cleanup**: Authorization sessions expire after 10 minutes; no automatic garbage collection implemented

## Future Enhancement Areas

- Token encryption (platform-specific keychain integration)
- Secrets Manager integration for `config/tokens.json`
- OpenTelemetry instrumentation
- Google Sandbox integration tests
- Garbage collection for expired `auth_sessions`
