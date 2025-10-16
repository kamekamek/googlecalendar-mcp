# Google Calendar MCP Server

[![CI](https://github.com/kamekamek/mcp-google-calendar/workflows/CI/badge.svg)](https://github.com/kamekamek/mcp-google-calendar/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-nightly-orange.svg)](https://www.rust-lang.org/)
[![MCP](https://img.shields.io/badge/MCP-0.8.1-green.svg)](https://modelcontextprotocol.io/)

Axum-based MCP bridge that lets coding agents read and write Google Calendar events. This README summarises the essentials in English and Japanese so you can set up, run, and test the server quickly.


---

## English Guide

### 1. Prerequisites
- Rust nightly toolchain (`rustup toolchain install nightly`). The project pins nightly via `rust-toolchain.toml`.
- Google Cloud project with the Google Calendar API enabled and an OAuth 2.0 **Web Application** client ID. Allow `http://localhost:8080/oauth/callback` (and `https://localhost:8443/proxy/oauth/callback` if you use the proxy).
- Optional for HTTPS/DCR testing: [`mkcert`](https://github.com/FiloSottile/mkcert) and [Caddy](https://caddyserver.com/).

### 2. Setup
1. Copy `.env.example` → `.env` and fill in:
   ```env
   APP__OAUTH__CLIENT_ID="<google client id>"
   APP__OAUTH__CLIENT_SECRET="<google client secret>"
   APP__SERVER__PUBLIC_URL="http://localhost:8080"
   ```
   Set `APP__SECURITY__USE_IN_MEMORY=true` when you do not want to persist tokens to disk.
2. Adjust `config/config.toml` if you need to change the bind address, public URL, or default calendar ID.
3. Ensure the token store path (default `config/tokens.json`) is writable. Protect it with filesystem permissions.

### 3. Google Cloud project setup (beginner friendly)
1. Go to the [Google Cloud console](https://console.cloud.google.com/) and create a new project (or select an existing one). Beginners can click the project drop-down in the top bar → **New Project** → give it a name and press **Create**.
2. Enable the Google Calendar API: open **APIs & Services → Library**, search for “Google Calendar API”, click it, then press **Enable**. The API tile shows as “Enabled” when finished.
3. Configure the OAuth consent screen (required even for testing):
   - Navigate to **APIs & Services → OAuth consent screen**.
   - Choose **External** user type unless you are on Google Workspace and only need internal users.
   - Fill in the App name, User support email, and Developer contact email. Upload a logo only if you want to customise the consent screen.
   - In **Scopes**, click **Add or Remove Scopes**, search for `.../auth/calendar`, select `https://www.googleapis.com/auth/calendar`, and save.
   - Under **Test users**, press **Add users** and enter every Google account that will run the MCP client during testing. Skip this step only if you plan to publish the app and pass Google’s verification.
   - Save changes at the bottom of the page.
4. Create OAuth credentials:
   - Open **APIs & Services → Credentials → Create credentials → OAuth client ID**.
   - Client type: **Web application** (required for localhost redirects).
   - Add the following **Authorised redirect URIs** (one per line):
     - `http://localhost:8080/oauth/callback`
     - `https://localhost:8443/proxy/oauth/callback` (needed when you enable the proxy/Caddy for Claude Code).
     - Add any production URLs you plan to deploy later, e.g. `https://mcp.example.com/oauth/callback`.
   - Click **Create** and copy the generated **Client ID** and **Client secret**. You can download the JSON or paste the values directly into your `.env`.
5. (Optional but recommended) Visit **APIs & Services → Domain verification** and verify your custom domain if you plan to expose the server publicly. This speeds up Google’s verification process once you move beyond test users.
6. Keep the app in “Testing” mode until you are ready for production. In testing mode, only the test users you added can sign in.

### 4. Start the server
```bash
cargo +nightly run
```
The server listens on `127.0.0.1:8080`. Visit `http://localhost:8080/oauth/authorize?user_id=demo-user` in a browser, sign in with Google, and the tokens for `demo-user` are stored automatically.

### 5. Configure an MCP client (`.mcp.json` example)
Add an entry pointing at the MCP SSE endpoint. When using Caddy, swap the URL for the HTTPS proxy.
```json
{
  "mcpServers": {
    "google_calendar": {
      "type": "sse",
      "url": "http://localhost:8080/mcp",
      "metadata": {
        "description": "Google Calendar bridge"
      }
    }
  }
}
```
Tools require a `user_id` argument. Claude Code prompts for it automatically; for other clients create a request payload such as:
```json
{
  "operation": "list",
  "user_id": "demo-user",
  "params": {
    "time_min": "2025-10-01T00:00:00Z",
    "time_max": "2025-10-07T23:59:59Z",
    "single_events": true,
    "order_by_start_time": true
  }
}
```

### 6. Verify with Claude Code (Claude Code CLI or claude.ai/code)
1. Start the server (and the HTTPS proxy if required).
2. In Claude Code, open **Settings → MCP Servers → Add MCP Server**.
3. Choose **Remote SSE**, set the URL to your proxy (`https://localhost:8443/mcp` when using Caddy) and keep the default connection headers.
4. Claude initiates OAuth via the proxy. Complete the browser flow once; the server logs `stored bearer token from headers` when the token is ingested.
5. Run `cl list-tools` (CLI) or the **Tools** palette to verify the four calendar tools are available. Invoke `google_calendar_list_events` with your `user_id` to confirm data flows end-to-end.

### 7. Local HTTPS with Caddy
1. Install `mkcert` and trust the local CA:
   ```bash
   mkcert -install
   mkcert localhost 127.0.0.1 ::1
   ```
   This creates `localhost+2.pem` and `localhost+2-key.pem` (already gitignored).
2. Update `.env` so the public URL matches the HTTPS endpoint:
   ```env
   APP__SERVER__PUBLIC_URL="https://localhost:8443"
   APP__PROXY__ENABLED=true
   ```
3. Use the provided `caddyfile` (reverse proxies `https://localhost:8443` to `http://127.0.0.1:8080`). Start it in a second shell:
   ```bash
   caddy run --config caddyfile
   ```
4. Point your MCP client at `https://localhost:8443/mcp` and repeat the OAuth flow. Claude Code now passes Dynamic Client Registration through the proxy.

---

## 日本語ガイド

### 1. 前提条件
- Rust nightly ツールチェーン（`rustup toolchain install nightly`）。リポジトリ同梱の `rust-toolchain.toml` が nightly を固定します。
- Google Calendar API が有効な Google Cloud プロジェクトと、OAuth 2.0 **ウェブアプリ** クライアント ID。リダイレクト URI に `http://localhost:8080/oauth/callback`（プロキシ利用時は `https://localhost:8443/proxy/oauth/callback` も）を追加してください。
- HTTPS/DCR をローカル検証する場合は `mkcert` と Caddy をインストールします。

### 2. セットアップ
1. `.env.example` を `.env` にコピーし、以下を設定します：
   ```env
   APP__OAUTH__CLIENT_ID="<Google クライアント ID>"
   APP__OAUTH__CLIENT_SECRET="<Google クライアント シークレット>"
   APP__SERVER__PUBLIC_URL="http://localhost:8080"
   ```
   トークンをファイル保存したくない場合は `APP__SECURITY__USE_IN_MEMORY=true` を追加します。
2. 必要に応じて `config/config.toml` で待受ポート、公開 URL、既定カレンダー ID を調整します。
3. 既定のトークン保存先（`config/tokens.json`）に書き込み権限があるか確認し、権限を制御してください。

### 3. Google Cloud の設定 (初心者向け)
1. [Google Cloud コンソール](https://console.cloud.google.com/) を開き、新しいプロジェクトを作成するか既存プロジェクトを選択します。右上のプロジェクト選択 → **新しいプロジェクト** → 名前を付けて **作成** を押します。
2. Google Calendar API を有効化します。**API とサービス → ライブラリ** で「Google Calendar API」を検索し、表示されたカードを開いて **有効にする** をクリックします。完了すると「有効」と表示されます。
3. OAuth 同意画面を設定します（テスト利用でも必須）:
   - **API とサービス → OAuth 同意画面** を開きます。
   - Google Workspace で社内利用のみの場合を除き、ユーザータイプは **外部** を選びます。
   - アプリ名、ユーザーサポート メール、デベロッパーの連絡先メールを入力します。ロゴは任意です。
   - **スコープ** で **スコープを追加または削除** を押し、`.../auth/calendar` を検索して `https://www.googleapis.com/auth/calendar` にチェックを入れ、保存します。
   - **テストユーザー** で **ユーザーを追加** を押し、テストに利用する Google アカウントをすべて登録します（公開前はここに登録したユーザーしかログインできません）。
   - 画面下部の **保存** を押します。
4. OAuth 認証情報を作成します。
   - **API とサービス → 認証情報 → 認証情報を作成 → OAuth クライアント ID** を選択します。
   - アプリケーションの種類は **ウェブ アプリケーション** を選びます。
   - **承認済みのリダイレクト URI** に以下を追加します（1行ずつ入力）:
     - `http://localhost:8080/oauth/callback`
     - `https://localhost:8443/proxy/oauth/callback` （Claude Code 用にプロキシ/Caddy を使用する場合）
     - 本番公開予定のドメインがある場合は `https://mcp.example.com/oauth/callback` のような URL も追加します。
   - **作成** を押し、表示された **クライアント ID** と **クライアント シークレット** をコピーします。JSON をダウンロードして `.env` に転記しても構いません。
5. （任意だが推奨）**API とサービス → ドメインの確認** からカスタムドメインを Search Console で所有確認しておくと、将来的な審査がスムーズです。
6. 公開準備が整うまではアプリの公開ステータスを「テスト」に維持してください。このモードでは登録済みテストユーザーのみがログインできます。

### 4. サーバー起動
```bash
cargo +nightly run
```
既定で `127.0.0.1:8080` で待受します。ブラウザで `http://localhost:8080/oauth/authorize?user_id=demo-user` を開き、Google にログインすると `demo-user` のトークンが保存されます。

### 5. MCP クライアント設定（`.mcp.json` 例）
SSE エンドポイントを指すエントリを追加します。Caddy 経由にする場合は URL を HTTPS に置き換えてください。
```json
{
  "mcpServers": {
    "google_calendar": {
      "type": "sse",
      "url": "http://localhost:8080/mcp",
      "metadata": {
        "description": "Google Calendar bridge"
      }
    }
  }
}
```
ツール呼び出し時は必ず `user_id` を指定します。例：
```json
{
  "operation": "list",
  "user_id": "demo-user",
  "params": {
    "time_min": "2025-10-01T00:00:00Z",
    "time_max": "2025-10-07T23:59:59Z",
    "single_events": true,
    "order_by_start_time": true
  }
}
```

### 6. Claude Code（claude.ai/code や CLI）での確認
1. サーバー（必要なら Caddy も）を起動します。
2. Claude Code の **Settings → MCP Servers → Add MCP Server** で **Remote SSE** を選択し、URL に HTTPS プロキシ（例：`https://localhost:8443/mcp`）を指定します。
3. 初回接続時にブラウザが立ち上がるので OAuth フローを完了します。サーバーログに `stored bearer token from headers` が出ればトークン取り込み成功です。
4. CLI なら `cl list-tools`、エディタ UI なら **Tools** パレットから `google_calendar_list_events` などを実行し、`user_id` を入力して動作を確認します。

### 7. Caddy を使ったローカル HTTPS
1. `mkcert -install` でローカル CA を登録し、`mkcert localhost 127.0.0.1 ::1` で証明書を生成します（`localhost+2.pem` / `localhost+2-key.pem` が作成されます）。
2. `.env` を以下のように更新して公開 URL とプロキシを有効化します：
   ```env
   APP__SERVER__PUBLIC_URL="https://localhost:8443"
   APP__PROXY__ENABLED=true
   ```
3. 付属の `caddyfile` は `https://localhost:8443` → `http://127.0.0.1:8080` に転送します。別のシェルで実行：
   ```bash
   caddy run --config caddyfile
   ```
4. MCP クライアントの接続先を `https://localhost:8443/mcp` に変更し、再度 OAuth を実施します。Claude Code の Dynamic Client Registration にも対応します。

---

## Handy Commands
- `cargo +nightly fmt` — format the codebase
- `cargo +nightly clippy -- -D warnings` — lint
- `cargo +nightly test` — run tests
- `make run` / `make fmt` / `make clippy` — shortcut targets defined in the Makefile

---

## Release Process (Automated)

This project uses [release-plz](https://release-plz.ieni.dev/) for fully automated releases. The workflow is triggered automatically when changes are merged to `main`.

### How It Works

1. **Automatic PR Creation**: When you push to `main`, the [release workflow](.github/workflows/release.yml) analyzes commits since the last release and creates a "Release PR" that:
   - Bumps version in `Cargo.toml` based on commit messages
   - Updates `CHANGELOG.md` with organized release notes
   - Groups changes by type (Added, Changed, Fixed, etc.)

2. **Review & Merge**: Review the release PR to verify:
   - Version bump is appropriate (patch/minor/major)
   - CHANGELOG entries are accurate
   - You can manually edit the PR to adjust version or changelog before merging

3. **Automatic Release**: When the release PR is merged:
   - Git tag (e.g., `v0.2.0`) is created automatically
   - GitHub Release is published with changelog notes
   - Release binaries are built for Linux and macOS (x86_64 and ARM64)

### Manual Release (if needed)

If you need to trigger a release manually or adjust the version:

```bash
# Install release-plz
cargo install release-plz

# Create release PR locally (dry-run)
release-plz release-pr --dry-run

# Create actual release PR
release-plz release-pr

# Or create a release directly (skips PR)
release-plz release
```

### Commit Message Tips

The automation works best with clear commit messages. Examples:

- `Add calendar sharing support` → grouped under "Added"
- `Fix token refresh race condition` → grouped under "Fixed"
- `Update event parsing logic` → grouped under "Changed"
- `Remove deprecated endpoints` → grouped under "Removed"

See [release-plz.toml](release-plz.toml) for the full list of commit patterns.

---

For detailed architecture notes and deployment guides, see the documents under `docs/` (e.g., `docs/design.md`, `docs/usage_guide.md`).
