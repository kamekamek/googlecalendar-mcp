# 利用ガイド

## 前提条件

1. Rust nightly ツールチェーンをインストールしておく（`rustup toolchain install nightly`）。`rmcp` が Edition 2024 を要求するため、stable ではビルドできません。リポジトリには `rust-toolchain.toml` が同梱されており、`cargo` 実行時に自動で nightly が選択されます。
2. Google Cloud Console でリダイレクト URI `http://localhost:8080/oauth/callback` を持つ OAuth クライアント（ウェブアプリ型）を作成しておく。
3. ルートにある `.env.example` を `.env` にコピーし、`APP__OAUTH__CLIENT_ID` / `APP__OAUTH__CLIENT_SECRET` を設定する。トークンをファイル保存したくない場合は `APP__SECURITY__USE_IN_MEMORY=true` を指定する。
4. 必要に応じて `config/config.toml` でバインドアドレスや既定カレンダー ID、ストレージモードを調整する。
5. Claude Code など DCR 対応クライアントを利用する場合は、`config/config.toml` で `proxy.enabled = true`（または `.env` で `APP__PROXY__ENABLED=true`）を有効化し、Google Cloud Console に `http://localhost:8080/proxy/oauth/callback`（必要に応じてカスタムドメイン）をリダイレクト URI として追加する。

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

- Remote MCP クライアント: `http://localhost:8080/mcp` に接続すると、最初の SSE イベントで POST 先 (`/mcp/message?sessionId=...`) が通知されます。
- HTTP 経由: `POST /mcp/tool` にツール名・引数を渡します。
- いずれの場合も必ず `user_id` を指定してください。`401 Unauthorized` が返った場合は、ブラウザで再度 OAuth 認証を行います。

### Remote MCP を公開する場合のインフラ選択

Claude Code などからインターネット越しに接続させるには、HTTPS エンドポイントと OAuth Dynamic Client Registration (DCR) を提供する必要があります。代表的な選択肢と特徴を以下にまとめます。

| 選択肢 | 概要 | 推奨度 | メリット | 注意点 |
| --- | --- | --- | --- | --- |
| **Google Cloud Run + Cloud Load Balancing** | Axum サーバーをコンテナ化して Cloud Run にデプロイし、Managed TLS で HTTPS 公開。DCR エンドポイントはサーバー内の `/proxy/oauth/...` を利用。 | ◎ | 完全マネージドでスケール自動 / 証明書自動更新 / Google OAuth との親和性が高い | 初回は `gcloud run deploy` などのセットアップが必要。`config/config.toml` の `server.public_url` を公開ドメインに合わせること。 |
| **Fly.io / Render / Railway** | Heroku 互換の PaaS にコンテナをそのままデプロイし、プラットフォーム付属の HTTPS を利用。 | ○ | セットアップが簡単で無料枠あり / 世界各リージョンに配置可能 | `config/tokens.json` を使う場合は永続ストレージ設定が必要。ドメイン設定と OAuth リダイレクト URI の更新を忘れずに。 |
| **Cloudflare + 任意の Compute** | Axum サーバーを任意の VM/サービスで動かし、Cloudflare Tunnel や Reverse Proxy で HTTPS と DCR を公開。 | △ | Cloudflare WAF/Access などが利用できる / 既存ドメイン管理と統合しやすい | トンネル常駐プロセスやバックエンド環境の確保が別途必要。 |
| **Compute Engine / EC2 などの VM 直ホスト** | VM に Axum と nginx/caddy をセットアップし、自前で HTTPS ・ DCR を提供。 | △ | 完全に自由な構成が組める | OS・証明書・スケールなど運用コストが高い。 |

推奨構成（Cloud Run）の詳細な手順や運用チェックリストは `docs/design.md` の「Remote MCP 公開戦略」を参照してください。

### Claude Code を利用する場合

- Claude Code CLI は OAuth 2.1 + DCR を必須としているため、上記のいずれかの方法で HTTPS + DCR を備えた公開エンドポイントを用意する。
- `.mcp.json` 等で公開 URL (`https://<your-domain>/mcp`) を指定し、初回接続時に Authorization ヘッダー経由でトークンが保存されているかをサーバーログ (`stored bearer token from headers`) で確認する。
- プロキシを用意できない場合は、STDIO 型 MCP サーバーや Claude Desktop のカスタムコネクタを利用してローカルで完結させる。 

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
