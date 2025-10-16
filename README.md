# Google Calendar MCP Server

[![CI](https://github.com/kamekamek/mcp-google-calendar/workflows/CI/badge.svg)](https://github.com/kamekamek/mcp-google-calendar/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-nightly-orange.svg)](https://www.rust-lang.org/)
[![MCP](https://img.shields.io/badge/MCP-0.8.1-green.svg)](https://modelcontextprotocol.io/)

**[English](README.md) | [日本語](README.ja.md)**

Axum-based MCP bridge that lets AI agents read and write Google Calendar events. This server provides OAuth-authenticated access to the Google Calendar API through the Model Context Protocol (MCP).

## Features

- 🔐 OAuth 2.0 Authentication with PKCE support
- 📅 Four Core Operations: List, Get, Create, and Update calendar events
- 🚀 Remote MCP Transport via Server-Sent Events (SSE)
- 🔄 Automatic Token Refresh handling
- 👥 Multi-User Support with per-user token isolation
- 🛡️ Security First: Event deletion intentionally disabled
- 🔌 Claude Code Compatible with optional OAuth 2.1 DCR proxy

## Quick Start

### Prerequisites

- Rust nightly toolchain (`rustup toolchain install nightly`)
- Google Cloud project with Calendar API enabled
- OAuth 2.0 Web Application credentials

### Installation

```bash
git clone https://github.com/kamekamek/mcp-google-calendar.git
cd mcp-google-calendar
cp .env.example .env
# Edit .env with your Google OAuth credentials
cargo +nightly run
```

Visit `http://localhost:8080/oauth/authorize?user_id=demo-user` to authorize.

## Google Cloud Setup

1. **Create a project** at [Google Cloud Console](https://console.cloud.google.com/)
2. **Enable Calendar API** (APIs & Services → Library)
3. **Configure OAuth consent screen** (APIs & Services → OAuth consent screen)
   - Add scope: `https://www.googleapis.com/auth/calendar`
   - Add test users
4. **Create OAuth credentials** (APIs & Services → Credentials → OAuth client ID)
   - Type: Web application
   - Redirect URIs:
     - `http://localhost:8080/oauth/callback`
     - `https://localhost:8443/proxy/oauth/callback` (for HTTPS mode)

## MCP Client Configuration

**.mcp.json example:**

```json
{
  "mcpServers": {
    "google_calendar": {
      "type": "sse",
      "url": "http://localhost:8080/mcp",
      "metadata": {
        "description": "Google Calendar MCP Server"
      }
    }
  }
}
```

### Claude Code Setup

1. Start server with HTTPS (see [Local HTTPS Setup](#local-https-setup))
2. Settings → MCP Servers → Add MCP Server
3. Type: Remote SSE, URL: `https://localhost:8443/mcp`
4. Complete OAuth flow
5. Test with `cl list-tools`

## Available Tools

All tools require `user_id` parameter.

- **`google_calendar_list_events`** - List calendar events with filtering
- **`google_calendar_get_event`** - Get a single event by ID
- **`google_calendar_create_event`** - Create a new event
- **`google_calendar_update_event`** - Update an existing event

See [CLAUDE.md](CLAUDE.md) for detailed parameter documentation.

## Local HTTPS Setup

For Claude Code compatibility:

```bash
# Install mkcert
mkcert -install
mkcert localhost 127.0.0.1 ::1

# Update .env
APP__SERVER__PUBLIC_URL="https://localhost:8443"
APP__PROXY__ENABLED=true

# Start Caddy
caddy run --config caddyfile
```

Update MCP client URL to `https://localhost:8443/mcp`.

## Development

```bash
cargo +nightly fmt                      # Format code
cargo +nightly clippy -- -D warnings    # Lint
cargo +nightly test                     # Run tests

# Makefile shortcuts
make run
make fmt
make clippy
```

## Configuration

Environment variables (`.env`):

```env
APP__OAUTH__CLIENT_ID="<google-client-id>"
APP__OAUTH__CLIENT_SECRET="<google-client-secret>"
APP__SERVER__PUBLIC_URL="http://localhost:8080"
APP__SECURITY__USE_IN_MEMORY=false    # Set true for ephemeral tokens
APP__PROXY__ENABLED=false             # Set true for Claude Code
```

See `config/config.toml` for full configuration options.

## Troubleshooting

### Token Refresh Issues
Refresh tokens are only issued on first authorization. Revoke at https://myaccount.google.com/permissions and re-authorize.

### EventDateTime Format
Use RFC3339: `"2025-10-15T06:00:00+09:00"` or object: `{"dateTime": "...", "timeZone": "Asia/Tokyo"}`

### HTTPS Not Working
- Verify certificates: `localhost+2.pem` and `localhost+2-key.pem`
- Check Caddy is running
- Confirm redirect URIs in Google Console include `/proxy/oauth/callback`

## Documentation

- [CLAUDE.md](CLAUDE.md) - Architecture & implementation details
- [docs/](docs/) - Deployment guides and usage patterns

## License

MIT License - see [LICENSE](LICENSE)

## Links

- [Model Context Protocol](https://modelcontextprotocol.io/)
- [Google Calendar API](https://developers.google.com/calendar/api)
- [Issue Tracker](https://github.com/kamekamek/mcp-google-calendar/issues)
