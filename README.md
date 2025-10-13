# Google Calendar MCP Server (Non-Delete Edition)

このリポジトリは、Model Context Protocol (MCP) 対応の Google カレンダー連携サーバーです。予定の参照・作成・更新ツールを提供し、削除機能は安全のため省いています。Claude や Cursor などの Remote MCP クライアントから接続できるほか、HTTP 経由のテスト用エンドポイントも用意しています。

## 1. Google Cloud Console での事前準備

1. **プロジェクト作成**（既存でも可）。
2. **Google Calendar API を有効化**
   - 「API とサービス」→「ライブラリ」→「Google Calendar API」→「有効にする」。
3. **OAuth 同意画面の設定**
   - ユーザータイプ（内部 or 外部）を選択し、アプリ名やサポートメールを入力。
   - スコープに `https://www.googleapis.com/auth/calendar` を追加（必要に応じて read-only 等に絞ることも可能）。
   - テストユーザー（外部アプリの場合）を登録。
4. **OAuth クライアント ID の作成**
   - 「認証情報」→「認証情報を作成」→「OAuth クライアント ID」。
   - アプリケーションの種類は「ウェブ アプリケーション」。
   - 承認済みのリダイレクト URI に `http://localhost:8080/oauth/callback` を追加（本番環境では HTTPS の本番 URL を追加）。
   - 発行された **クライアント ID** と **クライアント シークレット** を控える。

## 2. 環境変数・設定ファイル

1. ルートの `.env.example` をコピーして `.env` を作成し、以下を設定します。

   ```env
   APP__OAUTH__CLIENT_ID="<Google OAuth client ID>"
   APP__OAUTH__CLIENT_SECRET="<Google OAuth client secret>"
   APP__OAUTH__REDIRECT_URI="http://localhost:8080/oauth/callback"
   APP__SECURITY__TOKEN_STORE_PATH="config/tokens.json"
   APP__SECURITY__USE_IN_MEMORY="false"  # true にするとトークンをオンメモリ保持
   ```

2. `config/config.toml` でサーバーポートやデフォルトカレンダーを調整できます。
   - `server.bind_address` … サーバーのバインド先（例: `0.0.0.0:8080`）。
   - `google.calendar_id` … 既定で操作するカレンダー ID（未指定なら `primary`）。
   - `security.use_in_memory` … true にするとトークンをファイルに書き出さず、プロセス終了で破棄します。

## 3. 必須ツールチェーン

`rmcp` crate が Edition 2024 を要求するため **Rust nightly** が必要です。ルートに `rust-toolchain.toml` を置いているので、`cargo` 実行時に自動で nightly が選択されます。未インストールの場合は下記を先に実行してください。

```bash
rustup toolchain install nightly
```

## 4. ローカル起動手順

```bash
cargo +nightly run
```

起動するとデフォルトで `127.0.0.1:8080` で待ち受け、以下のエンドポイントを提供します。

| エンドポイント | 用途 |
| --- | --- |
| `GET /oauth/authorize` | 未認可ユーザーの OAuth 開始用 URL 返却 |
| `GET /oauth/callback` | Google OAuth コールバック（トークン保存） |
| `POST /mcp/tool` | HTTP 経由でツール操作をテストするための簡易 API |
| `GET /mcp/sse` | Remote MCP (SSE) ストリーム | 
| `POST /mcp/message?sessionId=...` | SSE 接続時に返される JSON-RPC 送信先 |

## 5. OAuth 認証フロー

1. 初回アクセスで `POST /mcp/tool` などからユーザー ID を指定してツール呼び出しを行うと、未認可の場合 `401` とともに認可 URL が返ります。
2. ブラウザでその URL を開き、Google アカウントで許可。
3. 認可後、Google が `http://localhost:8080/oauth/callback` にリダイレクトし、サーバー側でアクセストークンとリフレッシュトークンを保存します。
4. 以降は同じ `user_id` でツールを利用可能。トークンが失効した場合は自動でリフレッシュされます（リフレッシュ トークンが無い場合は再認可）。

## 6. Remote MCP クライアントからの利用

1. MCP クライアント（例: Claude Desktop の Custom MCP、Cursor など）に `http://localhost:8080/mcp/sse` を指定して接続。
2. 初回の SSE イベントで `message` 用 URL が通知されます。クライアントはそこに JSON-RPC を POST して通信を継続します。
3. 提供されるツール名:
   - `google_calendar_list_events`
   - `google_calendar_get_event`
   - `google_calendar_create_event`
   - `google_calendar_update_event`

HTTP 経由と同様に、ツール呼び出しには `user_id` が必須です。

## 7. トークンストレージ運用

- **ファイル保存 (デフォルト)**: `config/tokens.json` に暗号化なしで保存します。権限設定や暗号化レイヤーの追加は TODO として残してあります。
- **メモリ保持**: `.env` か `config.toml` で `APP__SECURITY__USE_IN_MEMORY=true` を設定すると、プロセス終了時にトークンも破棄されます。デモやテストに便利です。

## 8. テスト

```bash
cargo +nightly test
```

ユニットテストでは設定のデフォルト値やトークンストレージの基本挙動を検証しています。Google API への統合テストは未実装なので、必要に応じてモックサーバーや録画レスポンスを組み込んで下さい。

---

これで Google Cloud 側の準備からローカル起動、MCP クライアント接続の一連の流れが整います。問題や要望があれば `docs/` の設計資料も参照しつつ、Issue やドキュメントで補ってください。
