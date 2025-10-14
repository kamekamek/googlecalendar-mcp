# Google Calendar MCP サーバー — 設計メモ

## 概要

このサービスは Model Context Protocol (MCP) を介して Google カレンダーと安全に連携するためのブリッジを提供します。エージェントクライアントは OAuth 認証を開始し、更新可能なアクセストークンを保存しつつ、破壊的でない予定操作（`list` / `get` / `create` / `update`）を実行できます。誤操作によるデータ消失を避けるため、削除操作は意図的に実装していません。

## アーキテクチャ

- **HTTP ファサード（Axum 0.8）** — OAuth エンドポイント (`/oauth/authorize`, `/oauth/callback`) と JSON ベースの MCP 用エンドポイント (`/mcp/tool`) を提供します。
- **Remote MCP トランスポート（`rmcp::transport::sse_server`）** — `/mcp/sse` と `/mcp/message` で SSE ベースの MCP 通信を提供し、接続ごとに新しい `CalendarService` を立ち上げます。
- **OAuth モジュール（`src/oauth`）** — `oauth2` クレートを用いて PKCE チャレンジ、認可コード交換、トークン更新、トークン永続化を扱います。
- **トークンストレージ**
  - `FileTokenStorage` — `config/tokens.json` に永続化。`RwLock` でキャッシュを保護し、将来的に別ストレージへ差し替えられる構造です。
  - `InMemoryTokenStorage` — テスト用途や短時間のデモ向けにメモリのみで完結します。
- **Google カレンダー クライアント（`src/google_calendar`）** — `reqwest` を利用した薄いラッパーで、型安全なリクエスト／レスポンスと RFC3339 形式の時刻処理を提供します。
- **MCP サーバー（`src/mcp`）** — `#[tool]` で注釈したメソッドを RMCP `ServerHandler` として公開し、クライアント要求を Google API 呼び出しに変換します。HTTP 経由でのテスト用に `/mcp/tool` エンドポイントも残しています。

### オプション B: Dynamic Client Registration 対応の OAuth プロキシ

Claude Code など、OAuth 2.1 + Dynamic Client Registration (DCR) を必須とする MCP クライアントと連携する場合は、DCR に対応した HTTPS リバースプロキシを間に挟みます。プロキシは以下を提供する必要があります。

- `/.well-known/oauth-authorization-server` で `registration_endpoint` 等を含むメタデータを返す。
- `POST /register` で MCP クライアント向けのクライアント資格情報を払い出す（内部的には Google の固定クライアントを利用）。
- 認可エンドポイント／トークンエンドポイントを Google OAuth にブリッジする。
- HTTPS 終端（Let’s Encrypt や mkcert など）を行い、Claude Code から `https://` でアクセスできるようにする。

Mermaid による構成図:

```mermaid
graph LR
    subgraph Claude_Client[Claude Code / Desktop]
        C1[MCP クライアント]
    end

    C1 -- "HTTPS (SSE)" --> Proxy
    C1 -- "OAuth 2.1 + DCR" --> Proxy

    subgraph Proxy["OAuth プロキシ + DCR アダプタ"]
        P1["DCR エンドポイント<br/>(registration, authorization,<br/>token, metadata)"]
        P2["Google OAuth<br/>へのリバースプロキシ"]
    end

    P1 --> P2
    P2 -- "HTTPS" --> Google["Google OAuth 2.0"]

    Proxy -- "SSE/Websocket" --> Server["MCP Google Calendar Server"]
    Server -- "HTTPS" --> GoogleCal["Google Calendar API"]

    style Proxy fill:#f5f5f5,stroke:#888
    style Server fill:#f5f5f5,stroke:#888
```


運用時の注意点:

- プロキシ用のドメインと TLS 証明書を用意し、`https://mcp-proxy.example.com` のような URL で公開する。
- Google のクライアント ID / シークレットはプロキシ内部に保持し、外部（MCP クライアント）には DCR で払い出す資格情報のみを見せる。
- Claude Code や `.mcp.json` などの設定では、生の Axum サーバーではなくプロキシの URL を指定する。
- 複数の MCP サーバーを同じプロキシで扱う場合は、リソースメタデータを分離できるように設計する。

## リクエストフロー

1. エージェントが `/mcp/tool` を呼び出す。
2. トークンが未保存の場合、サーバーは `401` とともに認可 URL（PKCE の state を含む）を返す。
3. ユーザーが認可フローを完了すると、Google から `/oauth/callback` にリダイレクトされる。
4. コールバックでトークンを交換＆保存し、以後はツール呼び出しが成功する。

## セキュリティ上の考慮事項

- PKCE の verifier / state は 10 分で期限切れとなるようメモリに保持し、使用後は削除する。
- トークンはメモリキャッシュと `config/tokens.json` に保存。`encrypt_tokens` フラグで将来の暗号化対応を見込む。
- ログにはトークン値などの秘匿情報を出力しない。設定は環境変数や TOML で渡し、ハードコードを避ける。

## 今後の拡張案

- ファイルベースのトークン保存を Secrets Manager や OS キーチェーンに置き換える。
- プラットフォーム固有 API を用いたトークン暗号化。
- OpenTelemetry などによる構造化ログ／メトリクス出力。
- Google Sandbox を用いた統合テストやモック環境の整備。
- Streamable HTTP など、SSE 以外のトランスポートサポートに備えた抽象化。
