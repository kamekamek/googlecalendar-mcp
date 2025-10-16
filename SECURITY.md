# Security Policy

## Supported Versions

We provide security updates for the following versions:

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, please report security vulnerabilities using [GitHub Security Advisories](https://github.com/kamekamek/mcp-google-calendar/security/advisories/new).

You should receive a response within 48 hours. If for some reason you do not, please follow up via GitHub Issues to ensure we received your original message.

Please include the following information:
- Type of issue (e.g., buffer overflow, SQL injection, cross-site scripting)
- Full paths of source file(s) related to the issue
- Location of the affected source code (tag/branch/commit or direct URL)
- Step-by-step instructions to reproduce the issue
- Proof-of-concept or exploit code (if possible)
- Impact of the issue

## Security Best Practices

When deploying this server:
- Always use HTTPS in production
- Protect `config/tokens.json` with appropriate filesystem permissions (600 or 400)
- Never commit `.env`, `Secrets.toml`, or certificate files
- Regularly update dependencies with `cargo update`
- Run `cargo audit` periodically to check for known vulnerabilities
- Use strong OAuth client secrets (never use default/example values)
- Validate the `redirect_uri` matches your Google Cloud Console configuration
- Consider using secrets management services (AWS Secrets Manager, Google Secret Manager, etc.) for production

## Known Security Considerations

1. **Token Storage**: By default, tokens are stored in `config/tokens.json`. For production use, consider:
   - Setting `APP__SECURITY__USE_IN_MEMORY=true` for ephemeral environments
   - Implementing custom storage with encryption at rest
   - Using platform-specific keychain integration

2. **No Event Deletion**: This server intentionally omits delete operations to prevent accidental data loss

3. **User Isolation**: Each user's tokens are stored separately using `user_id` as the key

## Security Contact

For any security-related questions outside of vulnerability reports, please email the maintainers via GitHub Security Advisories or open a private discussion.
