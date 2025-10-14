# Google カレンダー MCP サーバー

Model Context Protocol (MCP) 準拠のエージェントから Google カレンダーを操作するためのサーバーです。予定の参照・作成・更新に対応し、誤操作による消失を防ぐため削除機能は実装していません。

## 特徴

- **MCP over SSE**: Claude Code や Cursor などの Remote MCP クライアントに対応
- **OAuth 2.0 認証**: PKCE フローによる安全なトークン管理
- **内蔵プロキシ**: Dynamic Client Registration (DCR) に対応し、Claude Code の OAuth 2.1 要件を満たす
- **トークン永続化**: ファイルまたはメモリにトークンを保存し、自動リフレッシュ対応

## デプロイ選択肢

Remote MCP として公開したい場合は、以下のホスティングを検討できます。詳細な比較と推奨手順は `docs/design.md` の「Remote MCP 公開戦略」で解説しています。

| 選択肢 | ひとことで | 推奨度 |
| --- | --- | --- |
| Google Cloud Run + Cloud Load Balancing | コンテナ化して完全マネージド運用 | ◎ |
| Shuttle.dev | `cargo shuttle deploy` で Rust サービスをデプロイ | ○ |
| VPS / 独自サーバー + Caddy | 自前で TLS / DCR を構成する柔軟な方法 | ○ |
| Azure Container Apps | Azure 上でマネージド運用 | △ |
| Cloudflare Workers / Tunnel | エッジ配信と既存インフラの組み合わせ | △ |

> **メモ**: Claude Code は OAuth 2.1 + DCR を必須とするため、どのプラットフォームでも HTTPS 終端と DCR エンドポイント (`/proxy/oauth/...`) を公開できることが前提になります。

## 利用シナリオ

このサーバーは2つの利用パターンに対応しています：

| パターン | 用途 | 必要な準備 |
|---------|------|-----------|
| **A. ローカル開発・テスト** | HTTP 経由でのツール動作確認、MCP プロトコルの開発 | Rust nightly + Google OAuth 設定 |
| **B. Claude Code 連携** | Claude Code CLI から Google カレンダーを操作 | 上記 + mkcert + Caddy + プロキシ有効化 |

---

## パターン A: ローカル開発・テスト用セットアップ

### 1. 前提条件

```bash
# Rust nightly のインストール（rmcp クレートが Edition 2024 を要求）
rustup toolchain install nightly
```

`rust-toolchain.toml` により、プロジェクト内では自動的に nightly が選択されます。

### 2. Google Cloud Console での準備

1. **プロジェクトを作成**（既存でも可）
2. **Google Calendar API を有効化**
   - 「API とサービス」→「ライブラリ」→「Google Calendar API」→「有効にする」
3. **OAuth 同意画面の設定**
   - ユーザータイプ（内部 / 外部）を選択
   - アプリ名・サポートメールを入力
   - スコープに `https://www.googleapis.com/auth/calendar` を追加
   - 外部アプリの場合はテストユーザーを登録
4. **OAuth クライアント ID の作成**
   - 「認証情報」→「認証情報を作成」→「OAuth クライアント ID」
   - 種別: **ウェブアプリケーション**
   - 承認済みリダイレクト URI: `http://localhost:8080/oauth/callback`
   - 発行されたクライアント ID / シークレットを控える

### 3. 環境変数の設定

```bash
# .env.example をコピー
cp .env.example .env
```

`.env` を編集して Google OAuth 認証情報を設定：

```env
APP__OAUTH__CLIENT_ID="<Google OAuth クライアント ID>"
APP__OAUTH__CLIENT_SECRET="<Google OAuth クライアントシークレット>"
APP__OAUTH__REDIRECT_URI="http://localhost:8080/oauth/callback"
APP__SECURITY__TOKEN_STORE_PATH="config/tokens.json"
APP__SECURITY__USE_IN_MEMORY="false"
```

### 4. サーバー起動

```bash
cargo +nightly run
```

デフォルトで `127.0.0.1:8080` で待ち受けます。

### 5. OAuth 認証

1. ブラウザで `http://localhost:8080/oauth/authorize?user_id=test-user` を開く
2. Google アカウントで同意
3. リダイレクト後、トークンが `config/tokens.json` に保存される

### 6. 動作確認

```bash
# 予定一覧を取得（HTTP 経由）
curl -X POST http://localhost:8080/mcp/tool \
  -H "Content-Type: application/json" \
  -d '{
    "operation": "list",
    "user_id": "test-user",
    "params": {
      "time_min": "2025-10-01T00:00:00Z",
      "time_max": "2025-10-31T23:59:59Z",
      "single_events": true,
      "order_by_start_time": true
    }
  }'
```

### テスト実行

```bash
cargo +nightly test
```

---

## パターン B: Claude Code 連携セットアップ

Claude Code は OAuth 2.1 + Dynamic Client Registration を必須とするため、HTTPS 終端と内蔵プロキシを有効化する必要があります。

### 1. パターン A の準備を完了

上記「パターン A」の手順 1〜3 を先に実行してください。

### 2. 追加ツールのインストール

#### mkcert（ローカル証明書生成）

```bash
# macOS
brew install mkcert
mkcert -install

# Ubuntu/Debian
sudo apt install libnss3-tools
curl -JLO https://dl.filippo.io/mkcert/latest?for=linux/amd64
chmod +x mkcert-*
sudo mv mkcert-* /usr/local/bin/mkcert
mkcert -install
```

#### Caddy（HTTPS リバースプロキシ）

```bash
# macOS
brew install caddy

# Ubuntu/Debian
sudo apt install -y debian-keyring debian-archive-keyring apt-transport-https
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | sudo gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' | sudo tee /etc/apt/sources.list.d/caddy-stable.list
sudo apt update
sudo apt install caddy

# その他の OS: https://caddyserver.com/docs/install
```

### 3. ローカル証明書の生成

```bash
# プロジェクトルートで実行
mkcert localhost 127.0.0.1 ::1
# → localhost+2.pem と localhost+2-key.pem が生成される
```

既に生成済みの場合はスキップしてください。

### 4. Google Cloud Console でリダイレクト URI を追加

OAuth クライアント ID の設定画面で、以下のリダイレクト URI を**追加**してください：

```
https://localhost:8443/proxy/oauth/callback
```

> **注意**: `http://localhost:8080/oauth/callback` も残しておくことで、パターン A と共存できます。

### 5. config.toml の編集

`config/config.toml` を開き、以下を確認・編集：

```toml
[server]
bind_address = "127.0.0.1:8080"
public_url = "https://localhost:8443"  # Caddy 経由の公開 URL

[oauth]
# .env で設定済みならそのまま
redirect_uri = "http://localhost:8080/oauth/callback"
```

**プロキシ設定を追加**（ファイル末尾に記述）：

```toml
[proxy]
enabled = true
redirect_path = "/proxy/oauth/callback"
```

### 6. Caddyfile の作成

```bash
# サンプルをコピー
cp caddyfile.example caddyfile
```

`caddyfile` を編集し、証明書ファイルのパスを実際の絶対パスに置き換えてください：

```caddyfile
https://localhost:8443 {
    tls /Users/yourname/mcp-server/localhost+2.pem /Users/yourname/mcp-server/localhost+2-key.pem
    reverse_proxy http://127.0.0.1:8080
}
```

> **重要**: `tls` 行のパスは必ず**絶対パス**で指定してください。相対パスは使えません。

### 7. サーバー起動（2つのプロセス）

#### ターミナル 1: Axum サーバー

```bash
cargo +nightly run
```

#### ターミナル 2: Caddy（HTTPS 終端）

```bash
# プロジェクトルートで実行
caddy run --config caddyfile
```

### 8. Claude Code で接続

`.mcp.json` または Claude Code のワークスペース設定に以下を追加：

```json
{
  "mcpServers": {
    "google_calendar": {
      "type": "sse",
      "url": "https://localhost:8443/mcp"
    }
  }
}
```

Claude Code から接続すると、自動的に OAuth フローが開始され、Authorization ヘッダー経由でトークンが保存されます。

### 9. 動作確認

Claude Code で以下のように操作できます：

```
今日の予定を教えて
```

サーバーログに `stored bearer token from headers` が表示されれば成功です。

---

## 設定ファイル構成

| ファイル | 用途 |
|---------|------|
| `.env` | OAuth クライアント ID/Secret など秘匿情報 |
| `config/config.toml` | サーバーのバインドアドレス、API ベース URL、プロキシ設定 |
| `config/tokens.json` | 保存された OAuth トークン（自動生成） |
| `caddyfile` | Caddy の HTTPS 終端設定 |

### 主要な設定項目

#### `.env` または環境変数

| 変数名 | 説明 | デフォルト |
|--------|------|-----------|
| `APP__OAUTH__CLIENT_ID` | Google OAuth クライアント ID | - |
| `APP__OAUTH__CLIENT_SECRET` | Google OAuth クライアントシークレット | - |
| `APP__OAUTH__REDIRECT_URI` | OAuth コールバック URI | `http://localhost:8080/oauth/callback` |
| `APP__SECURITY__USE_IN_MEMORY` | トークンをメモリのみに保存 | `false` |
| `APP__PROXY__ENABLED` | 内蔵 OAuth プロキシを有効化 | - |

#### `config/config.toml`

```toml
[server]
bind_address = "127.0.0.1:8080"        # サーバー待受アドレス
public_url = "https://localhost:8443"  # 公開 URL（OAuth リダイレクト用）

[google]
api_base = "https://www.googleapis.com/calendar/v3"
calendar_id = "primary"  # オプション: デフォルトカレンダー

[security]
token_store_path = "config/tokens.json"
encrypt_tokens = false  # 暗号化（未実装）
use_in_memory = false   # true でファイル保存しない

[proxy]
enabled = false          # Claude Code 利用時は true
redirect_path = "/proxy/oauth/callback"
```

---

## エンドポイント一覧

### OAuth エンドポイント

| エンドポイント | 用途 |
|---------------|------|
| `GET /oauth/authorize?user_id=<ID>` | 認可 URL を取得 |
| `GET /oauth/callback` | Google OAuth コールバック（トークン保存） |

### MCP エンドポイント

| エンドポイント | 用途 |
|---------------|------|
| `GET /mcp` | Remote MCP (SSE) ストリーム |
| `POST /mcp/message?sessionId=<ID>` | JSON-RPC メッセージ送信 |
| `POST /mcp/tool` | HTTP 経由の簡易テスト用 |

### プロキシエンドポイント（`proxy.enabled = true` 時のみ）

| エンドポイント | 用途 |
|---------------|------|
| `GET /.well-known/oauth-authorization-server` | DCR メタデータ |
| `POST /proxy/oauth/register` | 動的クライアント登録 |
| `GET /proxy/oauth/authorize` | プロキシ認可エンドポイント |
| `POST /proxy/oauth/token` | プロキシトークンエンドポイント |
| `GET /proxy/oauth/callback` | プロキシコールバック |

---

## 提供する MCP ツール

| ツール名 | 説明 | 必須パラメータ |
|---------|------|---------------|
| `google_calendar_list_events` | 予定一覧を取得 | `user_id` |
| `google_calendar_get_event` | 特定の予定を取得 | `user_id`, `event_id` |
| `google_calendar_create_event` | 予定を作成 | `user_id`, `summary`, `start`, `end` |
| `google_calendar_update_event` | 予定を更新 | `user_id`, `event_id` |

すべてのツールで `user_id` パラメータが必須です。未認証の場合は `401` が返り、認可 URL が提示されます。

---

## デプロイ・本番運用

ローカル開発以外の環境で Claude Code などから接続する場合は、以下の選択肢があります：

| デプロイ先 | 特徴 | 推奨度 |
|-----------|------|-------|
| **Google Cloud Run** | フルマネージド、自動スケール、証明書自動更新 | ◎ |
| **Fly.io / Render** | 簡単デプロイ、無料枠あり | ○ |
| **Cloudflare Tunnel** | 既存 VM + Cloudflare WAF | △ |
| **自前 VM (GCE/EC2)** | 完全な自由度、運用コスト高 | △ |

詳細な手順は `docs/usage_guide.md` および `docs/design.md` の「Remote MCP 公開戦略」を参照してください。

### 本番環境での注意点

- `server.public_url` を実際の公開ドメインに設定
- Google Cloud Console のリダイレクト URI を `https://<your-domain>/proxy/oauth/callback` に変更
- `config/tokens.json` のパーミッションを適切に設定（推奨: `600`）
- 将来的には Secrets Manager や OS キーチェーンへの移行を検討
- Caddy または Cloud Load Balancing で HTTPS 終端を実施

---

## トラブルシューティング

### 1. `failed to parse RFC3339 date-time string` エラー

**原因**: `start` / `end` パラメータが RFC3339 形式の文字列として渡されていない。

**解決策**: 以下の形式で渡してください：

```json
{
  "start": "2025-10-15T06:00:00+09:00",
  "end": "2025-10-15T07:00:00+09:00"
}
```

JSON オブジェクトではなく、**文字列**として渡します。

### 2. `user 'xxx' is not authorized; complete OAuth flow` エラー

**原因**: 指定した `user_id` のトークンが保存されていない。

**解決策**: ブラウザで `/oauth/authorize?user_id=<ID>` にアクセスし、OAuth 認証を完了してください。

### 3. `access token for user 'xxx' is expired and lacks a refresh token` エラー

**原因**: トークンの有効期限が切れ、リフレッシュトークンが保存されていない。

**解決策**: 再度 OAuth 認証を実行してください。初回認証時に `offline` アクセスが要求されていることを確認してください。

### 4. Caddy が起動しない（ポート競合）

**原因**: 既に 8443 ポートが使用されている。

**解決策**:
```bash
# ポート使用状況を確認
lsof -i :8443

# 別のポートを使う場合は caddyfile を編集
```

### 5. Claude Code が接続できない

**チェックリスト**:
- [ ] `config/config.toml` で `proxy.enabled = true` になっているか
- [ ] Caddy が起動しているか（`https://localhost:8443` にアクセス可能か）
- [ ] Google Cloud Console のリダイレクト URI に `https://localhost:8443/proxy/oauth/callback` が登録されているか
- [ ] ローカル証明書が有効か（`mkcert -install` を実行済みか）
- [ ] サーバーログに `stored bearer token from headers` が表示されるか

### 6. 別アカウントで再認証したい

**原因**: 指定した `user_id` に紐づくトークンが残っており、新しいアカウントで認証しても切り替わらない。

**解決策**:

```bash
curl -X DELETE https://<サーバードメイン>/oauth/token/<user_id>
```

204 No Content が返れば削除成功です。削除後に `/oauth/authorize?user_id=<user_id>` を再度開くと、新しいアカウントで OAuth フローが実行されます。

---

## 関連ドキュメント

- [利用ガイド](docs/usage_guide.md) - 詳細な利用方法とインフラ選択
- [API リファレンス](docs/api_reference.md) - エンドポイント仕様
- [設計ドキュメント](docs/design.md) - アーキテクチャと運用戦略
- [CLAUDE.md](CLAUDE.md) - Claude Code 向けコードベースガイド

---

## ライセンス

このプロジェクトは個人利用・学習目的で作成されています。本番運用時はセキュリティ要件を十分に検討してください。
