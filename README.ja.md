# Google Calendar MCP Server

[![CI](https://github.com/kamekamek/mcp-google-calendar/workflows/CI/badge.svg)](https://github.com/kamekamek/mcp-google-calendar/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-nightly-orange.svg)](https://www.rust-lang.org/)
[![MCP](https://img.shields.io/badge/MCP-0.8.1-green.svg)](https://modelcontextprotocol.io/)

**[English](README.md) | [日本語](README.ja.md)**

AIエージェントがGoogle Calendarの予定を読み書きできるようにするAxumベースのMCPブリッジです。Model Context Protocol (MCP) を通じて、Google Calendar API への OAuth 認証済みアクセスを提供します。

## 特徴

- 🔐 PKCE 対応の OAuth 2.0 認証
- 📅 4つのコア操作: 予定の一覧取得、取得、作成、更新
- 🚀 Server-Sent Events (SSE) によるリモート MCP トランスポート
- 🔄 自動トークンリフレッシュ処理
- 👥 ユーザーごとのトークン分離によるマルチユーザー対応
- 🛡️ セキュリティ優先: 予定削除機能は意図的に無効化
- 🔌 Claude Code 対応 (OAuth 2.1 DCR プロキシ対応)

## クイックスタート

### 前提条件

- Rust nightly ツールチェーン (`rustup toolchain install nightly`)
- Google Calendar API が有効な Google Cloud プロジェクト
- OAuth 2.0 ウェブアプリケーション認証情報

### インストール

```bash
git clone https://github.com/kamekamek/mcp-google-calendar.git
cd mcp-google-calendar
cp .env.example .env
# .env に Google OAuth 認証情報を設定
cargo +nightly run
```

ブラウザで `http://localhost:8080/oauth/authorize?user_id=demo-user` を開いて認証します。

## Google Cloud のセットアップ

1. **プロジェクトを作成** ([Google Cloud Console](https://console.cloud.google.com/))
2. **Calendar API を有効化** (API とサービス → ライブラリ)
3. **OAuth 同意画面を設定** (API とサービス → OAuth 同意画面)
   - スコープを追加: `https://www.googleapis.com/auth/calendar`
   - テストユーザーを追加
4. **OAuth 認証情報を作成** (API とサービス → 認証情報 → OAuth クライアント ID)
   - 種類: ウェブアプリケーション
   - リダイレクト URI:
     - `http://localhost:8080/oauth/callback`
     - `https://localhost:8443/proxy/oauth/callback` (HTTPS モード用)

## MCP クライアント設定

**.mcp.json の例:**

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

### Claude Code のセットアップ

1. HTTPS でサーバーを起動 ([ローカル HTTPS セットアップ](#ローカル-https-セットアップ)を参照)
2. Settings → MCP Servers → Add MCP Server
3. タイプ: Remote SSE、URL: `https://localhost:8443/mcp`
4. OAuth フローを完了
5. `cl list-tools` でテスト

## 利用可能なツール

すべてのツールで `user_id` パラメータが必要です。

- **`google_calendar_list_events`** - フィルタリング付きで予定を一覧取得
- **`google_calendar_get_event`** - ID で予定を1件取得
- **`google_calendar_create_event`** - 新しい予定を作成
- **`google_calendar_update_event`** - 既存の予定を更新

詳細なパラメータ仕様は [CLAUDE.md](CLAUDE.md) を参照してください。

## ローカル HTTPS セットアップ

Claude Code と連携するには:

```bash
# mkcert をインストール
mkcert -install
mkcert localhost 127.0.0.1 ::1

# .env を更新
APP__SERVER__PUBLIC_URL="https://localhost:8443"
APP__PROXY__ENABLED=true

# Caddy を起動
caddy run --config caddyfile
```

MCP クライアントの URL を `https://localhost:8443/mcp` に変更します。

## 開発

```bash
cargo +nightly fmt                      # コードフォーマット
cargo +nightly clippy -- -D warnings    # リント
cargo +nightly test                     # テスト実行

# Makefile ショートカット
make run
make fmt
make clippy
```

## 設定

環境変数 (`.env`):

```env
APP__OAUTH__CLIENT_ID="<google-client-id>"
APP__OAUTH__CLIENT_SECRET="<google-client-secret>"
APP__SERVER__PUBLIC_URL="http://localhost:8080"
APP__SECURITY__USE_IN_MEMORY=false    # true でメモリ内トークン保存
APP__PROXY__ENABLED=false             # true で Claude Code 対応
```

完全な設定オプションは `config/config.toml` を参照してください。

## トラブルシューティング

### トークンリフレッシュの問題
リフレッシュトークンは初回認証時のみ発行されます。https://myaccount.google.com/permissions でアクセスを取り消して再認証してください。

### EventDateTime フォーマット
RFC3339 形式を使用: `"2025-10-15T06:00:00+09:00"` またはオブジェクト形式: `{"dateTime": "...", "timeZone": "Asia/Tokyo"}`

### HTTPS が動作しない
- 証明書を確認: `localhost+2.pem` と `localhost+2-key.pem`
- Caddy が起動しているか確認
- Google Console のリダイレクト URI に `/proxy/oauth/callback` が含まれているか確認

## ドキュメント

- [CLAUDE.md](CLAUDE.md) - アーキテクチャと実装の詳細
- [docs/](docs/) - デプロイガイドと使用パターン

## ライセンス

MIT License - 詳細は [LICENSE](LICENSE) を参照

## リンク

- [Model Context Protocol](https://modelcontextprotocol.io/)
- [Google Calendar API](https://developers.google.com/calendar/api)
- [Issue Tracker](https://github.com/kamekamek/mcp-google-calendar/issues)
