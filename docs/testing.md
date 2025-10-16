# Testing Guide

## Running Tests

### Unit Tests
```bash
# Run all tests
make test
# or
cargo +nightly test

# Run with output
cargo +nightly test -- --nocapture

# Run specific test
cargo +nightly test test_name
```

### Integration Tests
```bash
# Run only integration tests
cargo +nightly test --test '*'
```

## Test Structure

```
tests/
├── integration/
│   └── oauth_flow_test.rs  # OAuth flow integration tests
└── unit/
    └── config_test.rs       # Configuration tests
```

## Manual Testing

### 1. OAuth Flow Test
```bash
# Start the server
make run

# Open browser
open http://localhost:8080/oauth/authorize?user_id=test-user

# Complete Google OAuth consent
# Verify redirect to /oauth/callback with success message
```

### 2. List Events Test
```bash
# Using HTTP tool endpoint
curl -X POST http://localhost:8080/mcp/tool \
  -H "Content-Type: application/json" \
  -d '{
    "operation": "list",
    "user_id": "test-user",
    "params": {
      "time_min": "2025-10-01T00:00:00Z",
      "time_max": "2025-10-31T23:59:59Z",
      "max_results": 10
    }
  }'
```

### 3. Create Event Test
```bash
curl -X POST http://localhost:8080/mcp/tool \
  -H "Content-Type: application/json" \
  -d '{
    "operation": "create",
    "user_id": "test-user",
    "params": {
      "summary": "Test Event",
      "start": "2025-10-20T10:00:00+09:00",
      "end": "2025-10-20T11:00:00+09:00",
      "description": "Created via MCP server"
    }
  }'
```

### 4. Remote MCP Transport Test
```bash
# Connect to SSE endpoint
curl -N http://localhost:8080/mcp

# Send JSON-RPC request
curl -X POST http://localhost:8080/mcp/message \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/list",
    "params": {}
  }'
```

## Mocking Google Calendar API

For automated testing without real Google API calls, consider:

### Option 1: Use `mockito` or `wiremock`
```toml
[dev-dependencies]
mockito = "1.0"
```

### Option 2: Test Trait Implementation
Create a test implementation of the calendar client:
```rust
#[cfg(test)]
pub struct MockCalendarClient {
    // Mock responses
}

#[cfg(test)]
impl MockCalendarClient {
    pub fn new_with_responses(responses: Vec<Response>) -> Self {
        // ...
    }
}
```

## Testing OAuth Proxy (DCR Mode)

```bash
# Enable proxy
export APP__PROXY__ENABLED=true

# Test DCR metadata endpoint
curl https://localhost:8443/.well-known/oauth-authorization-server

# Test client registration
curl -X POST https://localhost:8443/proxy/oauth/register \
  -H "Content-Type: application/json" \
  -d '{
    "client_name": "Test Client",
    "redirect_uris": ["https://example.com/callback"]
  }'
```

## Test Coverage

Check test coverage using `cargo-tarpaulin`:
```bash
cargo install cargo-tarpaulin
cargo +nightly tarpaulin --out Html
```

## Common Test Scenarios

1. **Token Expiry**: Test automatic refresh when access token expires
2. **Invalid Credentials**: Verify 401 response with authorization URL
3. **Concurrent Requests**: Test multiple users accessing simultaneously
4. **Token Storage**: Verify file persistence and in-memory modes
5. **Error Handling**: Test Google API error responses

## Debugging Tests

Enable debug logging:
```bash
RUST_LOG=debug cargo +nightly test -- --nocapture
```
