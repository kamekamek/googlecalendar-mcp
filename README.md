# Google Calendar MCP Server

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-nightly-orange.svg)](https://www.rust-lang.org/)
[![MCP](https://img.shields.io/badge/MCP-0.8.1-green.svg)](https://modelcontextprotocol.io/)

**[English](README.md) | [日本語](README.ja.md)**

An Axum-based MCP server that enables AI agents to read and write Google Calendar events. Access the Google Calendar API through OAuth authentication via the Model Context Protocol (MCP).

## Features

- 🔐 OAuth 2.0 Authentication with PKCE support
- 📅 Four operations: List, Get, Create, and Update calendar events
- 🚀 Remote MCP Transport via Server-Sent Events (SSE)
- 🔄 Automatic token refresh
- 👥 Multi-user support with per-user token isolation
- 🛡️ Security-first: Delete operations intentionally disabled
- 🔌 Full Claude Code compatibility

## Setup Guide

### 1. Google Cloud Project Setup

#### 1-1. Create a Project

1. Go to [Google Cloud Console](https://console.cloud.google.com/)
2. Click the project selector dropdown at the top
3. Click "New Project"
4. Enter a project name (e.g., `mcp-calendar-server`) and click "Create"

#### 1-2. Enable Google Calendar API

1. Navigate to "APIs & Services" → "Library" from the sidebar
2. Search for "Google Calendar API"
3. Click "Google Calendar API"
4. Click "Enable"
5. Wait for "API enabled" confirmation

#### 1-3. Configure OAuth Consent Screen

1. Go to "APIs & Services" → "OAuth consent screen"
2. **User Type**: Select "External" and click "Create"
   - Select "Internal" only if using Google Workspace for internal users only
3. **App information**:
   - App name: `MCP Calendar Server` (or any name)
   - User support email: Select your email
   - Developer contact information: Enter your email
4. Click "Save and Continue"

#### 1-4. Add Scopes

1. Click "Add or Remove Scopes"
2. Filter by `calendar`
3. Check `https://www.googleapis.com/auth/calendar`
4. Click "Update" → "Save and Continue"

#### 1-5. Add Test Users

1. In "Test users" section, click "ADD USERS"
2. Enter your Google account email address
3. Click "Add" → "Save and Continue"
4. Click "Back to Dashboard"

> **Important**: In testing mode, only users added here can sign in.

#### 1-6. Create OAuth Credentials

1. Go to "APIs & Services" → "Credentials"
2. Click "Create Credentials" → "OAuth client ID"
3. **Application type**: Select "Web application"
4. **Name**: `MCP Calendar OAuth Client` (or any name)
5. Under **Authorized redirect URIs**, click "Add URI" and add:
   - `http://localhost:8080/oauth/callback`
   - `https://localhost:8443/proxy/oauth/callback`
6. Click "Create"
7. Copy and save the displayed "Client ID" and "Client secret"
   - ❗Save these credentials - you'll need them later

### 2. Local Environment Setup

#### 2-1. Clone Repository and Install Rust

```bash
# Clone repository
git clone https://github.com/kamekamek/mcp-google-calendar.git
cd mcp-google-calendar

# Install Rust nightly (if not already installed)
rustup toolchain install nightly
```

#### 2-2. Configure Environment Variables

```bash
# Copy .env.example to .env
cp .env.example .env
```

Edit the `.env` file with your Google OAuth credentials:

```env
APP__OAUTH__CLIENT_ID="<paste your client ID here>"
APP__OAUTH__CLIENT_SECRET="<paste your client secret here>"
APP__SERVER__PUBLIC_URL="https://localhost:8443"
APP__PROXY__ENABLED=true
```

### 3. Install and Run Caddy

#### 3-1. Generate Certificates with mkcert

```bash
# Install mkcert (using Homebrew)
brew install mkcert

# Install local CA
mkcert -install

# Generate certificates for localhost
mkcert localhost 127.0.0.1 ::1
# → Creates localhost+2.pem and localhost+2-key.pem
```

#### 3-2. Install and Run Caddy

```bash
# Install Caddy
brew install caddy

# Run Caddy (in a separate terminal)
caddy run --config caddyfile
```

Keep this terminal open as Caddy needs to continue running.

#### 3-3. Start MCP Server

Open a new terminal:

```bash
cd mcp-google-calendar
cargo +nightly run
```

Server starts on `127.0.0.1:8080`. Keep this terminal open as well.

### 4. Claude Code Configuration

#### 4-1. Configure .mcp.json

Edit `.mcp.json` (create if it doesn't exist):

```json
{
  "mcpServers": {
    "google_calendar": {
      "type": "sse",
      "url": "https://localhost:8443/mcp",
      "metadata": {
        "description": "Google Calendar MCP Server"
      }
    }
  }
}
```

#### 4-2. Launch Claude Code

```bash
# Start Claude Code CLI
claude
```

After startup, run:

```
/mcp
```

The MCP connection menu will appear.

#### 4-3. Authentication Flow

1. Select `google_calendar` from the MCP server list
2. Click "Authenticate" button
3. Browser automatically opens with Google OAuth screen
4. Sign in with the Google account added as test user
5. Review app permissions and click "Allow"
6. When browser shows "Authentication complete", return to Claude Code
7. Available tools list will be displayed after connection

### 5. Verification

Try this in Claude Code:

```
Show me my calendar events for this week
```

Or directly invoke tools:

```
/tools
```

Select `google_calendar_list_events` from the list and execute.

## Available Tools

All tools require a `user_id` parameter (automatically set by Claude Code).

### google_calendar_list_events
Retrieve a list of calendar events.

**Parameters:**
- `time_min`: Start time filter (RFC3339: `2025-10-20T00:00:00+09:00`)
- `time_max`: End time filter
- `max_results`: Maximum events to return (1-2500)
- `calendar_id`: Calendar ID (defaults to "primary")

### google_calendar_get_event
Fetch a single event by ID.

**Parameters:**
- `event_id`: Event ID (required)
- `calendar_id`: Calendar ID (defaults to "primary")

### google_calendar_create_event
Create a new calendar event.

**Parameters:**
- `summary`: Event title (required)
- `start`: Start time (required)
- `end`: End time (required)
- `description`: Description (optional)
- `location`: Location (optional)

**Date/time format:**
```
"2025-10-20T10:00:00+09:00"
```

### google_calendar_update_event
Update an existing event.

**Parameters:**
- `event_id`: Event ID (required)
- `summary`, `start`, `end`, `description`, `location`: Fields to update (optional)

## Troubleshooting

### Authentication Error

**Cause**: Attempting to sign in with a Google account not added as test user

**Solution**:
1. Google Cloud Console → OAuth consent screen → Test users
2. Add the Google account you want to use

### Token Refresh Error

**Cause**: Refresh tokens are only issued on first authorization

**Solution**:
1. Visit https://myaccount.google.com/permissions
2. Find "MCP Calendar Server" and remove it
3. Re-authenticate through Claude Code

### HTTPS Error

**Cause**: Certificates missing or Caddy not running

**Solution**:
```bash
# Check certificates exist
ls localhost+2*.pem
# → Should see localhost+2.pem and localhost+2-key.pem

# Check if Caddy is running
lsof -i :8443
# → Should see caddy process
```

### EventDateTime Format Error

Use RFC3339 format:
```
"2025-10-20T10:00:00+09:00"
```

Or object format:
```json
{
  "dateTime": "2025-10-20T10:00:00+09:00",
  "timeZone": "Asia/Tokyo"
}
```

## For Developers

### Build and Test

```bash
# Format
cargo +nightly fmt

# Lint
cargo +nightly clippy -- -D warnings

# Test
cargo +nightly test
```

### Configuration

See `config/config.toml` for complete configuration options.

Override with environment variables:
- `APP__OAUTH__CLIENT_ID`
- `APP__OAUTH__CLIENT_SECRET`
- `APP__SERVER__PUBLIC_URL`
- `APP__SECURITY__USE_IN_MEMORY` (true/false)
- `APP__PROXY__ENABLED` (true/false)

## License

MIT License - see [LICENSE](LICENSE) for details

## Links

- [Model Context Protocol](https://modelcontextprotocol.io/)
- [Google Calendar API](https://developers.google.com/calendar/api)
- [Issue Tracker](https://github.com/kamekamek/mcp-google-calendar/issues)
