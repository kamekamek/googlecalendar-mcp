# 利用ガイド

## 前提条件

1. Rust nightly ツールチェーンをインストールしておく（`rustup toolchain install nightly`）。`rmcp` が Edition 2024 を要求するため、stable ではビルドできません。リポジトリには `rust-toolchain.toml` が同梱されており、`cargo` 実行時に自動で nightly が選択されます。
2. Google Cloud Console でリダイレクト URI `http://localhost:8080/oauth/callback` を持つ OAuth クライアント（ウェブアプリ型）を作成しておく。
3. ルートにある `.env.example` を `.env` にコピーし、`APP__OAUTH__CLIENT_ID` / `APP__OAUTH__CLIENT_SECRET` を設定する。トークンをファイル保存したくない場合は `APP__SECURITY__USE_IN_MEMORY=true` を指定する。
4. 必要に応じて `config/config.toml` でバインドアドレスや既定カレンダー ID、ストレージモードを調整する。

## サーバーの起動

```bash
cargo +nightly run
```

設定された `bind_address`（デフォルト `127.0.0.1:8080`）で待ち受けます。

## ユーザー認証の手順

1. エージェントから `GET /oauth/authorize?user_id=<ユーザーID>` を呼び出す。
2. レスポンスに含まれる `authorize_url` をブラウザで開いて Google にログイン。
3. 同意後、Google が `/oauth/callback` にリダイレクトし、指定した `user_id` のトークンを保存します。

トークンは既定で `config/tokens.json` に保存されます。権限や暗号化などは環境に合わせて管理してください。

## MCP ツールの呼び出し

- Remote MCP クライアント: `http://localhost:8080/mcp/sse` に接続すると、最初の SSE イベントで POST 先 (`/mcp/message?sessionId=...`) が通知されます。
- HTTP 経由: `POST /mcp/tool` にツール名・引数を渡します。
- いずれの場合も必ず `user_id` を指定してください。`401 Unauthorized` が返った場合は、ブラウザで再度 OAuth 認証を行います。

### Claude Code を利用する場合（OAuth プロキシ経由）

- Claude Code CLI は OAuth 2.1 + Dynamic Client Registration を必須としているため、Google OAuth を直接指定するとエラーになります。
- `docs/design.md` の「オプション B」を参考に、HTTPS で DCR に対応したプロキシを構築し、その URL を `.mcp.json` 等に設定してください。
- プロキシが用意できない場合は、STDIO 型 MCP サーバーや Claude Desktop のコネクタ機能を利用する方法を推奨します。

### 例: 予定一覧取得

```json
{
  "operation": "list",
  "user_id": "demo-user",
  "params": {
    "time_min": "2025-10-13T00:00:00Z",
    "time_max": "2025-10-20T00:00:00Z",
    "single_events": true,
    "order_by_start_time": true
  }
}
```

## テスト

- `cargo +nightly test` で設定値・シリアライズ・トークンストレージの単体テストを実行できます。
- 統合テストとして Google API を直接呼ぶ場合は、`google.api_base` をモックサーバーに差し替えるなど環境を整えてから実施してください。
