# Google カレンダー MCP サーバー（削除操作なし版）

このリポジトリは、Model Context Protocol (MCP) に準拠したエージェント用の Google カレンダー連携サーバーです。予定の参照・作成・更新に対応し、誤操作による消失を避けるため削除機能は実装していません。Claude / Cursor などの Remote MCP クライアント、または HTTP 経由での動作確認が可能です。

## 1. Google Cloud Console での準備

1. **プロジェクトを作成**（既存でも可）。
2. **Google Calendar API を有効化** — 「API とサービス」→「ライブラリ」→「Google Calendar API」→「有効にする」。
3. **OAuth 同意画面の設定**
   - ユーザータイプ（内部 / 外部）を選択し、アプリ名・サポートメールを入力。
   - スコープに `https://www.googleapis.com/auth/calendar` を追加（必要に応じて read-only 等に絞ってもよい）。
   - 外部アプリの場合はテストユーザーを登録。
4. **OAuth クライアント ID の作成**
   - 「認証情報」→「認証情報を作成」→「OAuth クライアント ID」。
   - 種別を「ウェブアプリケーション」に設定し、承認済みリダイレクト URI に `http://localhost:8080/oauth/callback` を追加（本番運用時は HTTPS の本番 URL を登録）。
   - 発行されたクライアント ID / シークレットを控える。

## 2. 環境変数・設定ファイル

1. ルートにある `.env.example` を `.env` にコピーし、以下を設定します。

   ```env
   APP__OAUTH__CLIENT_ID="<Google OAuth client ID>"
   APP__OAUTH__CLIENT_SECRET="<Google OAuth client secret>"
   APP__OAUTH__REDIRECT_URI="http://localhost:8080/oauth/callback"
   APP__SECURITY__TOKEN_STORE_PATH="config/tokens.json"
   APP__SECURITY__USE_IN_MEMORY="false"  # true でメモリ保存のみ
   ```

2. `config/config.toml` では以下を主に調整します。
   - `server.bind_address` … サーバー待受アドレス（例：`0.0.0.0:8080`）。
   - `google.calendar_id` … 既定で操作するカレンダー ID（未指定なら `primary`）。
   - `security.use_in_memory` … true でトークンをファイルへ書き出さず、プロセス終了時に破棄します。

## 3. 必要なツールチェーン

`rmcp` クレートが Edition 2024 を要求するため **Rust nightly** が必須です。`rust-toolchain.toml` を同梱しているので、プロジェクト内で `cargo` を実行すると自動的に nightly が選択されます。未インストールの場合は下記を実行してください。

```bash
rustup toolchain install nightly
```

## 4. 起動方法

```bash
cargo +nightly run
```

デフォルトで `127.0.0.1:8080` で待ち受け、以下のエンドポイントを提供します。

| エンドポイント | 用途 |
| --- | --- |
| `GET /oauth/authorize` | 未認可ユーザー向けに認可 URL を返却 |
| `GET /oauth/callback` | Google OAuth コールバック（トークン保存） |
| `POST /mcp/tool` | HTTP 経由の簡易テスト用 MCP エンドポイント |
| `GET /mcp/sse` | Remote MCP (SSE) ストリーム |
| `POST /mcp/message?sessionId=...` | SSE 接続時に通知される JSON-RPC 送信先 |

## 5. OAuth 認可フロー

1. `POST /mcp/tool` 等で初回アクセスすると、未認可の場合は `401` と認可 URL が返ります。
2. ブラウザで認可 URL を開き、Google アカウントで同意します。
3. 認可後 `http://localhost:8080/oauth/callback` にリダイレクトされ、サーバーがトークンを保存します。
4. 同じ `user_id` で再度ツールを呼ぶと、Google カレンダーの操作が可能になります。トークン期限切れの場合は自動でリフレッシュします（リフレッシュトークンが無い場合は再度認可が必要）。

## 6. Remote MCP クライアントからの利用

1. Claude Desktop や Cursor など、MCP クライアントから `http://localhost:8080/mcp/sse` に接続します。
2. 初回の SSE イベントで `message` 用 URL が通知されるので、そこへ JSON-RPC を POST して通信します。
3. 提供ツール一覧
   - `google_calendar_list_events`
   - `google_calendar_get_event`
   - `google_calendar_create_event`
   - `google_calendar_update_event`

HTTP 経由と同様に、ツール呼び出し時には `user_id` を必ず指定してください。

## 7. トークン保存戦略

- **ファイル保存（デフォルト）**: `config/tokens.json` に保存します。権限設定や暗号化レイヤーは今後の課題です。
- **メモリ保存**: `.env` または `config/config.toml` で `APP__SECURITY__USE_IN_MEMORY=true` を設定すると、プロセス終了時にトークンが破棄されます。

## 8. テスト

```bash
cargo +nightly test
```

ユニットテストでは設定のデフォルト値とトークンストレージの基本挙動を検証しています。Google API を呼ぶ統合テストは未実装なので、必要に応じてモックや録画レスポンスを組み込んでください。

## 9. Claude Code と連携するための OAuth プロキシ構成

Claude Code (CLI) は OAuth 2.1 + Dynamic Client Registration を必須とするため、Google OAuth を直接利用すると `dynamic client registration` 関連のエラーが発生します。Claude Code から利用する場合は、DCR に対応した HTTPS プロキシを挟んで要件を吸収する必要があります。

1. **DCR 対応プロキシを構築**
   - `/.well-known/oauth-authorization-server` で `registration_endpoint` など必要なメタデータを返す
   - `POST /register` で MCP クライアント向けのクライアント資格を払い出し
   - 認可 / トークンエンドポイントを Google OAuth にブリッジ
   - HTTPS 証明書を設定 （Let’s Encrypt, mkcert など）

2. **MCP サーバーへの中継**
   - プロキシから Axum サーバーの `/mcp/sse` と `/mcp/message` へリバースプロキシ
   - サーバー本体は `cargo +nightly run` で従来どおり起動

3. **Claude Code 側の設定例**
   ```json
   {
     "mcpServers": {
       "google_calendar": {
         "type": "sse",
         "url": "https://mcp-proxy.example.com/mcp"
       }
     }
   }
   ```

構成図や詳細は `docs/design.md` を参照してください。プロキシが Google OAuth とやり取りし、Claude Code には DCR 対応のメタデータだけを見せる形になります。プロキシ構築が難しい場合は、Claude Desktop のコネクタ機能、もしくは STDIO 型 MCP サーバー（`workspace-mcp` 等）を利用してください。

---

以上で Google Cloud 側の準備からサーバー起動、MCP クライアントからの利用方法までを網羅しています。補足や更新が必要な場合は `docs/` 配下の資料も併せて参照してください。
