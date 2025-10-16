# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2025-10-16

### Added
- Initial release of Google Calendar MCP Server
- OAuth 2.0 authorization code flow with PKCE support
- Remote MCP transport over Server-Sent Events (SSE)
- Four core MCP tools:
  - `google_calendar_list_events` - List events with filtering and pagination
  - `google_calendar_get_event` - Fetch single event by ID
  - `google_calendar_create_event` - Create new calendar events
  - `google_calendar_update_event` - Update existing events
- Optional OAuth 2.1 Dynamic Client Registration (DCR) proxy for Claude Code compatibility
- Pluggable token storage (file-based and in-memory modes)
- Bearer token ingestion from Authorization headers
- Comprehensive documentation in `docs/` directory
- Deployment guides for Google Cloud Run and Shuttle.dev
- HTTP endpoints for testing and integration
- Automatic token refresh handling
- Multi-user support with per-user token isolation

### Security
- Event deletion intentionally disabled to prevent accidental data loss
- Secrets protection via `.gitignore`
- PKCE (Proof Key for Code Exchange) implementation

[Unreleased]: https://github.com/kamekamek/mcp-google-calendar/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/kamekamek/mcp-google-calendar/releases/tag/v0.1.0
