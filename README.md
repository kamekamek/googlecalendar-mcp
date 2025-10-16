# Google Calendar MCP Server

Model Context Protocol (MCP) bridge that lets AI coding agents read and write Google Calendar events with minimal setup. The server exposes both Remote MCP over Server-Sent Events and JSON-based HTTP helpers so you can integrate it from Claude Code, Codex CLI, or your own tooling.

> [!IMPORTANT]
> Event deletion is intentionally disabled to guard against accidental data loss. The tools only support listing, reading, creating, and updating entries.

## Table of Contents
- [Overview](#overview)
- [Features](#features)
- [Project Layout](#project-layout)
- [Quick Start (Local Development)](#quick-start-local-development)
- [Remote MCP Clients](#remote-mcp-clients)
- [Configuration](#configuration)
- [Available MCP Tools](#available-mcp-tools)
- [HTTP Endpoints](#http-endpoints)
- [Development & Testing](#development--testing)
- [Troubleshooting](#troubleshooting)
- [Further Reading](#further-reading)

## Overview
Google Calendar requires OAuth 2.0 and careful token storage, which can be painful to implement in every agent. This project centralizes the integration: it handles PKCE authorization, refreshes access tokens, and exposes typed MCP tools backed by the official Google Calendar API. Remote MCP clients connect over SSE (`/mcp`), while HTTP-friendly routes under `/mcp/tool` make it easy to smoke-test or automate from scripts.

## Features
- Remote MCP transport powered by `rmcp::transport::sse_server`, with graceful shutdown and per-session service instances.
- OAuth 2.0 authorization code flow with PKCE, plus optional OAuth 2.1 Dynamic Client Registration (DCR) facade for Claude Code and other strict clients.
- Pluggable token storage: JSON file persistence by default or in-memory mode for demos/tests, with automatic refresh support.
- Bearer token ingestion from incoming headers so hosted deployments can accept pre-authorized requests without a full OAuth round-trip.
- Deployment-friendly tooling including Shuttle support, Cloud Run guides, and a Caddy reverse-proxy example for local HTTPS.
- Structured tracing via `tracing`/`tracing-subscriber` to help diagnose OAuth exchanges and Google API calls.

## Project Layout
```
mcp-server/
├── config/              # Default TOML configuration and token store path
├── docs/                # Detailed design, usage, and deployment guides
├── src/                 # Application code (config, OAuth, MCP tools, proxy)
├── tests/               # Integration and unit test scaffolding
├── Makefile             # Helper targets for local workflows
└── Shuttle.toml         # Optional Shuttle deployment configuration
```

## Quick Start (Local Development)
### Prerequisites
- Rust nightly toolchain (`rustup toolchain install nightly`). The `rmcp` crate currently requires Edition 2024 features.
- Google Cloud project with the Google Calendar API enabled and an OAuth client ID (Web application type) that allows `http://localhost:8080/oauth/callback`.
- Optional: `mkcert` and Caddy if you want to try HTTPS + DCR locally.

### Setup
1. Duplicate `.env.example` to `.env` and fill in your Google OAuth credentials.
2. Adjust `config/config.toml` if you need a different bind address, public URL, or default calendar ID.
3. Ensure the token store path (`config/tokens.json` by default) is writable and protected with appropriate filesystem permissions.

> [!NOTE]
> Configuration follows this precedence: `config/config.toml` → `config/config.local.toml` → environment variables prefixed with `APP__`. When `APP__SECURITY__USE_IN_MEMORY=true`, tokens stay in RAM and the file store is ignored.

### Run the server
```bash
make run
# or
cargo +nightly run --bin mcp_google_calendar
```
The server listens on `127.0.0.1:8080` by default and logs the public URL it will advertise to OAuth clients.

### Authorize a user
1. Open `http://localhost:8080/oauth/authorize?user_id=demo-user` in a browser (or call it from your agent).
2. Follow the Google consent screen. After redirecting to `/oauth/callback`, the access and refresh tokens will be persisted.
3. Repeat the flow with a different `user_id` when you need to separate accounts.

### Call a tool (HTTP helper)
```bash
curl -X POST http://localhost:8080/mcp/tool \
  -H "Content-Type: application/json" \
  -d '{
    "operation": "list",
    "user_id": "demo-user",
    "params": {
      "time_min": "2025-10-01T00:00:00Z",
      "time_max": "2025-10-31T23:59:59Z",
      "single_events": true,
      "order_by_start_time": true
    }
  }'
```
A 401 response includes an authorization URL when the user still needs to complete OAuth.

## Remote MCP Clients
When you want Claude Code, Codex CLI, or another remote MCP consumer to connect over HTTPS:
- Enable the OAuth proxy by setting `APP__PROXY__ENABLED=true` (or `proxy.enabled = true` in `config/config.toml`). This exposes DCR endpoints such as `/.well-known/oauth-authorization-server` and `/proxy/oauth/register`.
- Set `server.public_url` to the externally reachable `https://` origin and ensure the same URL is registered as an OAuth redirect (`<public-url>/proxy/oauth/callback`).
- Terminate TLS with your platform of choice (Caddy, Cloud Run Load Balancer, Shuttle TLS, etc.). SSE endpoints require generous idle timeouts.
- Provide the MCP client with the `/mcp` URL. The proxy automatically registers DCR clients, exchanges codes with Google, and forwards refreshed tokens back to the core server.

Common hosting paths:
- **Google Cloud Run** — Containerize the app, deploy behind managed TLS, and map secrets via environment variables. `docs/deployment_cloudrun.md` walks through the full setup.
- **Shuttle.dev** — Enable the `shuttle` feature (`cargo +nightly shuttle run`) to run and deploy serverless Rust services. See `docs/deployment_shuttle.md` for required secrets and logs commands.
- **Self-hosted with Caddy** — Reuse the provided `caddyfile` template to proxy `https://localhost:8443` to the Axum server and satisfy OAuth 2.1 DCR checks from Claude Code.

> [!TIP]
> Remote clients can send fresh bearer and refresh tokens via `Authorization` and the `x-mcp-oauth-*` headers. The server will persist them automatically, so you can rotate credentials without re-running the browser flow.

## Configuration
Key settings are available via environment variables (prefix `APP__`) or the TOML files. Highlights:

| Setting | Description |
| --- | --- |
| `APP__SERVER__BIND_ADDRESS` | Socket address for the HTTP/SSE server (default `127.0.0.1:8080`). |
| `APP__SERVER__PUBLIC_URL` | Base URL advertised to OAuth clients; must match registered redirect origins. |
| `APP__OAUTH__CLIENT_ID` / `CLIENT_SECRET` | Google OAuth credentials used for the upstream authorization code exchange. |
| `APP__OAUTH__REDIRECT_URI` | Direct callback path for local development (`/oauth/callback`). Overrides require the same value in Google Cloud. |
| `APP__GOOGLE__CALENDAR_ID` | Optional default calendar; otherwise `primary` is used. |
| `APP__SECURITY__TOKEN_STORE_PATH` | File path for persisted tokens. |
| `APP__SECURITY__USE_IN_MEMORY` | Set to `true` to keep tokens ephemeral (testing/demo). |
| `APP__PROXY__ENABLED` | Enables the OAuth 2.1 + DCR proxy endpoints. |

## Available MCP Tools
| Tool name | Purpose | Required parameters |
| --- | --- | --- |
| `google_calendar_list_events` | List events within an optional time range, pagination, and search filters. | `user_id` |
| `google_calendar_get_event` | Fetch a single event by ID (optional calendar override). | `user_id`, `event_id` |
| `google_calendar_create_event` | Create an event with summary, start/end, attendees, reminders, etc. | `user_id`, `summary`, `start`, `end` |
| `google_calendar_update_event` | Patch an existing event without deleting it. | `user_id`, `event_id` plus any mutable fields |

Each tool enforces OAuth presence. Unauthorized calls return an MCP error that includes a fresh authorization URL and PKCE verifier.

## HTTP Endpoints
| Method & path | Description |
| --- | --- |
| `GET /health` | Basic health probe returning `ok`. |
| `GET /oauth/authorize` | Starts the PKCE flow and returns the authorize URL + state. |
| `GET /oauth/callback` | Completes OAuth and persists tokens. |
| `DELETE /oauth/token/{user_id}` | Clears stored tokens for re-authentication. |
| `POST /mcp/tool` | JSON helper for invoking MCP tools without the SSE transport. |
| `GET /mcp` / `POST /mcp/message` | Remote MCP transport over SSE + JSON-RPC ingress. |
| `/.well-known/*`, `/proxy/oauth/*` | OAuth 2.1 DCR metadata, registration, authorization, and token exchange (when proxy is enabled).

## Development & Testing
- `make run` / `cargo +nightly run` — start the local server.
- `make test` — execute unit tests on the nightly toolchain.
- `make fmt` / `make clippy` — format and lint the codebase.
- `make shuttle-deploy` — deploy with Shuttle once secrets are configured.

Logs use the `tracing` crate. Set `RUST_LOG=info` (or `debug`) to control verbosity during development.

### AI-Assisted Development
This project integrates both **Claude Code** and **OpenAI Codex** for development assistance:

- **Claude Code** (`@claude`) — Interactive code reviews, architecture discussions, and refactoring guidance via GitHub issues/PRs
- **Codex CI** — Automated CI/CD analysis that diagnoses build failures, test errors, and clippy warnings on every push/PR

To enable Codex CI integration, add your `OPENAI_API_KEY` to repository secrets. See [`docs/codex-integration.md`](docs/codex-integration.md) for detailed setup instructions and usage examples.

## Troubleshooting
> [!WARNING]
> `failed to parse RFC3339 date-time string` means `start` and `end` must be provided as RFC3339 strings (e.g., `2025-10-15T06:00:00+09:00`).

> [!TIP]
> If you receive `user 'xyz' is not authorized`, repeat the `/oauth/authorize` request and complete the browser flow. Deleting `/oauth/token/{user_id}` resets stored credentials.

> [!NOTE]
> Claude Code and Codex insist on HTTPS endpoints. When testing locally, use the bundled `caddyfile` together with `mkcert` to satisfy TLS and DCR requirements.

## Further Reading
- `docs/usage_guide.md` — step-by-step walkthrough of the OAuth flow and agent integration patterns.
- `docs/api_reference.md` — detailed specification for each HTTP endpoint and MCP payload.
- `docs/design.md` — architecture notes, deployment strategies, and security considerations.
- `docs/deployment_cloudrun.md` & `docs/deployment_shuttle.md` — platform-specific deployment guides.
- `CLAUDE.md` — tips for using this project from Claude Code workspaces.
