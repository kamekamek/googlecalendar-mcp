# Security Guidelines

## Overview

This document outlines security best practices for deploying and maintaining the Google Calendar MCP Server.

## Authentication & Authorization

### OAuth 2.0 Security

1. **Client Credentials Protection**
   - Never commit `APP__OAUTH__CLIENT_ID` or `APP__OAUTH__CLIENT_SECRET`
   - Use environment variables or secrets management services
   - Rotate credentials periodically (every 90 days recommended)

2. **Redirect URI Validation**
   - Always register exact redirect URIs in Google Cloud Console
   - Never use wildcards in production
   - Use HTTPS for all redirect URIs in production

3. **PKCE Implementation**
   - This server uses PKCE (Proof Key for Code Exchange) to prevent authorization code interception
   - Code verifier is stored temporarily (10 minutes) in memory
   - Never log or expose code verifier or code challenge

### Token Management

1. **Access Token Storage**
   ```bash
   # File permissions for token storage
   chmod 600 config/tokens.json

   # Owner-only read/write
   -rw------- 1 user user tokens.json
   ```

2. **Token Lifecycle**
   - Access tokens expire after 1 hour (Google default)
   - Refresh tokens persist until revoked
   - Automatic refresh happens transparently
   - Failed refresh requires re-authorization

3. **Storage Modes**
   - **File Storage** (default): Persists tokens to `config/tokens.json`
     - Suitable for development and single-instance production
     - Requires proper filesystem permissions
   - **In-Memory Storage**: Ephemeral tokens (lost on restart)
     - Enable with `APP__SECURITY__USE_IN_MEMORY=true`
     - Suitable for stateless/serverless environments
     - Users must re-authenticate after server restart

## Network Security

### TLS/HTTPS

**Production deployments MUST use HTTPS:**

```bash
# Local development with mkcert
mkcert localhost 127.0.0.1 ::1
mkcert -install

# Use Caddy for reverse proxy
caddy reverse-proxy --from https://localhost:8443 --to http://localhost:8080
```

### CORS Configuration

- This server does not implement CORS by default
- If exposing to browser clients, add CORS middleware:
  ```rust
  use tower_http::cors::CorsLayer;

  .layer(CorsLayer::new()
      .allow_origin(["https://example.com".parse().unwrap()])
      .allow_methods([Method::GET, Method::POST]))
  ```

## Input Validation

### User ID Validation
```rust
// Validate user_id to prevent path traversal
fn validate_user_id(user_id: &str) -> Result<(), Error> {
    if user_id.contains("..") || user_id.contains("/") {
        return Err(Error::InvalidInput);
    }
    Ok(())
}
```

### Event Data Sanitization
- Summary and description fields are passed to Google Calendar API
- Google's API validates and sanitizes HTML in descriptions
- No additional XSS protection needed at this layer

## Deployment Security

### Environment Variables
```bash
# Use secrets management
# AWS
aws secretsmanager get-secret-value --secret-id mcp-oauth-client-id

# Google Cloud
gcloud secrets versions access latest --secret="oauth-client-id"

# Kubernetes
kubectl create secret generic oauth-creds \
  --from-literal=client-id=YOUR_ID \
  --from-literal=client-secret=YOUR_SECRET
```

### Container Security
```dockerfile
# Run as non-root user
USER 1000:1000

# Read-only root filesystem
VOLUME ["/app/config"]

# Drop capabilities
SECURITY_OPT no-new-privileges
```

### Cloud Run Security
```bash
# Deploy with minimal permissions
gcloud run deploy mcp-calendar \
  --no-allow-unauthenticated \
  --service-account=mcp-sa@project.iam.gserviceaccount.com \
  --ingress=internal-and-cloud-load-balancing
```

## Audit & Monitoring

### Logging
- Enable structured logging: `RUST_LOG=info`
- Never log tokens or secrets
- Log authentication events:
  - OAuth flow initiation
  - Token refresh
  - Authorization failures

```rust
tracing::info!(user_id = %user_id, "User authorized successfully");
// ❌ DO NOT: tracing::debug!("Token: {}", token);
```

### Security Monitoring
```bash
# Check for vulnerabilities
cargo audit

# Update dependencies
cargo update

# CI/CD integration
# Add to .github/workflows/ci.yml
- name: Security audit
  uses: rustsec/audit-check@v1
```

## Threat Model

### Threats Mitigated
1. ✅ Authorization code interception (PKCE)
2. ✅ Accidental data deletion (delete operations disabled)
3. ✅ Token leakage in logs (tokens never logged)
4. ✅ Path traversal in user_id (validation required)

### Residual Risks
1. ⚠️ Token storage encryption (filesystem-level only)
2. ⚠️ No rate limiting (rely on Google's API limits)
3. ⚠️ No audit trail for event modifications
4. ⚠️ No token revocation endpoint (use Google's revocation)

## Incident Response

### Token Compromise
1. Revoke tokens in Google Cloud Console
2. Rotate OAuth client credentials
3. Delete `config/tokens.json`
4. Force all users to re-authenticate

### Vulnerability Disclosure
Follow `SECURITY.md` to report vulnerabilities via GitHub Security Advisories.

## Compliance

### Data Handling
- This server does not store calendar event data
- Only OAuth tokens are persisted
- User data is proxied directly to/from Google Calendar API

### GDPR Considerations
- Users can revoke access via Google Account settings
- Deleting `config/tokens.json` removes all stored tokens
- No analytics or tracking implemented

## Security Checklist

Before deploying to production:

- [ ] HTTPS enabled with valid certificate
- [ ] `tokens.json` has 600 permissions
- [ ] OAuth client secret stored in secrets manager
- [ ] `RUST_LOG` set to `info` or `warn` (not `debug`)
- [ ] Redirect URIs registered in Google Cloud Console
- [ ] Dependabot or `cargo audit` enabled
- [ ] Monitoring/alerting configured
- [ ] Backup strategy for `tokens.json` (if using file storage)
- [ ] Rate limiting configured (if needed)
- [ ] Security contact/policy documented in SECURITY.md
