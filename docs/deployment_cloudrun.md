# Google Cloud Run へのデプロイガイド

Google Cloud Run は完全マネージドなコンテナプラットフォームで、Google Calendar API との親和性が高く、本番運用に最適です。

## 前提条件

- Google Cloud アカウント
- gcloud CLI のインストール: https://cloud.google.com/sdk/docs/install
- Docker のインストール

## 1. Dockerfile の作成

プロジェクトルートに `Dockerfile` を作成：

```dockerfile
# ビルドステージ
FROM rust:1.83-slim as builder

WORKDIR /app

# 依存関係のキャッシュ
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release || true
RUN rm -rf src

# アプリケーションのビルド
COPY . .
RUN cargo build --release

# 実行ステージ
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/mcp_google_calendar /app/mcp_google_calendar
COPY config/config.toml /app/config/config.toml

# Cloud Run のデフォルトポート
ENV PORT=8080
ENV APP__SERVER__BIND_ADDRESS="0.0.0.0:8080"
ENV APP__PROXY__ENABLED="true"

EXPOSE 8080

CMD ["/app/mcp_google_calendar"]
```

## 2. .dockerignore の作成

```
target/
.git/
.env
config/tokens.json
*.pem
caddyfile
```

## 3. Google Cloud プロジェクトのセットアップ

```bash
# Google Cloud にログイン
gcloud auth login

# プロジェクト作成（既存の場合はスキップ）
gcloud projects create mcp-calendar-prod --name="MCP Calendar"

# プロジェクトを選択
gcloud config set project mcp-calendar-prod

# 必要な API を有効化
gcloud services enable \
  run.googleapis.com \
  artifactregistry.googleapis.com \
  calendar.googleapis.com
```

## 4. Artifact Registry にイメージをプッシュ

```bash
# Artifact Registry リポジトリを作成
gcloud artifacts repositories create mcp-calendar \
  --repository-format=docker \
  --location=asia-northeast1 \
  --description="MCP Google Calendar"

# Docker 認証を設定
gcloud auth configure-docker asia-northeast1-docker.pkg.dev

# イメージをビルド
docker build -t asia-northeast1-docker.pkg.dev/mcp-calendar-prod/mcp-calendar/server:latest .

# イメージをプッシュ
docker push asia-northeast1-docker.pkg.dev/mcp-calendar-prod/mcp-calendar/server:latest
```

## 5. Cloud Run 用の環境変数

Cloud Run ではローカル向けの `.env` ではなく、以下の環境変数を `gcloud run deploy` で設定します。

- `APP__OAUTH__CLIENT_ID` / `APP__OAUTH__CLIENT_SECRET`
- `APP__OAUTH__REDIRECT_URI`
  - Proxy を有効化する場合は `https://<service-url>/proxy/oauth/callback`
- `APP__SERVER__PUBLIC_URL`
  - Cloud Run の公開 URL かカスタムドメインを指定
- `APP__PROXY__ENABLED`（`true` 推奨）
- `APP__SERVER__BIND_ADDRESS`（`0.0.0.0:8080`）
- `APP__SECURITY__USE_IN_MEMORY`
  - 本番では `false` 推奨（永続ストレージは後述）

ローカル用途の値は `.env.example` を参照してください。

## 6. Cloud Run にデプロイ

```bash
gcloud run deploy mcp-calendar \
  --image=asia-northeast1-docker.pkg.dev/mcp-calendar-prod/mcp-calendar/server:latest \
  --platform=managed \
  --region=asia-northeast1 \
  --allow-unauthenticated \
  --port=8080 \
  --timeout=300 \
  --set-env-vars="APP__OAUTH__CLIENT_ID=<your-client-id>" \
  --set-env-vars="APP__OAUTH__CLIENT_SECRET=<your-client-secret>" \
  --set-env-vars="APP__OAUTH__REDIRECT_URI=https://mcp-calendar-<hash>-an.a.run.app/proxy/oauth/callback" \
  --set-env-vars="APP__SERVER__PUBLIC_URL=https://mcp-calendar-<hash>-an.a.run.app" \
  --set-env-vars="APP__PROXY__ENABLED=true" \
  --set-env-vars="APP__SECURITY__USE_IN_MEMORY=false"
```

> **注意**: `<hash>` はデプロイ後に払い出される URL に含まれます。初回デプロイ後に URL を確認し、環境変数とリダイレクト URI を更新してください。

## 7. カスタムドメインの設定（オプション）

```bash
# ドメインマッピングを作成
gcloud run domain-mappings create \
  --service=mcp-calendar \
  --domain=mcp.example.com \
  --region=asia-northeast1
```

手順に従って DNS レコード（A または CNAME）を設定すると、TLS 証明書が自動発行されます。

## 8. Google Cloud Console の設定

OAuth クライアント ID のリダイレクト URI に以下を追加：

```
https://mcp-calendar-<hash>-an.a.run.app/proxy/oauth/callback
```

または、カスタムドメインを使う場合：

```
https://mcp.example.com/proxy/oauth/callback
```

## 9. トークンストレージの永続化（推奨）

Cloud Run はステートレスなので、`config/tokens.json` はコンテナ再起動時に失われます。以下の選択肢を検討してください：

### オプション A: Cloud Storage

```bash
# バケットを作成
gsutil mb -l asia-northeast1 gs://mcp-calendar-tokens

# サービスアカウントに権限を付与
gcloud projects add-iam-policy-binding mcp-calendar-prod \
  --member="serviceAccount:<service-account>@mcp-calendar-prod.iam.gserviceaccount.com" \
  --role="roles/storage.objectAdmin"
```

`src/oauth/storage.rs` に `CloudStorageTokenStorage` を実装：

```rust
pub struct CloudStorageTokenStorage {
    bucket: String,
    client: GoogleCloudStorageClient,
}

impl TokenStorage for CloudStorageTokenStorage {
    async fn fetch(&self, user_id: &str) -> Result<Option<TokenInfo>> {
        // gs://bucket/tokens/{user_id}.json から読み込み
    }

    async fn persist(&self, user_id: &str, token: &TokenInfo) -> Result<()> {
        // gs://bucket/tokens/{user_id}.json に保存
    }
}
```

### オプション B: Firestore

```bash
# Firestore を有効化
gcloud firestore databases create --region=asia-northeast1
```

`Cargo.toml` に依存を追加：

```toml
firestore = "0.41"
```

## 10. Claude Code で接続

`.mcp.json` に以下を追加：

```json
{
  "mcpServers": {
    "google_calendar": {
      "type": "sse",
      "url": "https://mcp-calendar-<hash>-an.a.run.app/mcp"
    }
  }
}
```

## 10. ログとモニタリング

```bash
# ログをストリーミング
gcloud run services logs tail mcp-calendar --region=asia-northeast1

# メトリクスを確認
gcloud run services describe mcp-calendar --region=asia-northeast1
```

Cloud Console の「Cloud Run」→「mcp-calendar」で以下を確認できます：
- リクエスト数・レイテンシ
- エラー率
- CPU・メモリ使用率

## トラブルシューティング

### 1. コンテナが起動しない

```bash
# ローカルでテスト
docker run -p 8080:8080 \
  -e APP__OAUTH__CLIENT_ID=<id> \
  -e APP__OAUTH__CLIENT_SECRET=<secret> \
  asia-northeast1-docker.pkg.dev/mcp-calendar-prod/mcp-calendar/server:latest
```

### 2. SSE 接続がタイムアウトする

Cloud Run のデフォルトタイムアウトは 5 分です。`--timeout=300` で最大値（5分）に設定してください。

### 3. トークンが保存されない

`APP__SECURITY__USE_IN_MEMORY=true` にすると、トークンはメモリに保存され、コンテナ再起動時に失われます。本番環境では Cloud Storage/Firestore を使ってください。

## 料金

- **無料枠**: 月 200万リクエスト、36万 vCPU 秒、18万 GiB 秒
- **従量課金**: リクエスト $0.40/百万、vCPU $0.00002400/秒、メモリ $0.00000250/GiB 秒

個人利用なら無料枠で十分です。詳細: https://cloud.google.com/run/pricing

## CI/CD（オプション）

GitHub Actions で自動デプロイ：

`.github/workflows/deploy.yml`:

```yaml
name: Deploy to Cloud Run

on:
  push:
    branches: [main]

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: google-github-actions/setup-gcloud@v2
        with:
          service_account_key: ${{ secrets.GCP_SA_KEY }}
          project_id: mcp-calendar-prod

      - name: Build and Push
        run: |
          gcloud auth configure-docker asia-northeast1-docker.pkg.dev
          docker build -t asia-northeast1-docker.pkg.dev/mcp-calendar-prod/mcp-calendar/server:${{ github.sha }} .
          docker push asia-northeast1-docker.pkg.dev/mcp-calendar-prod/mcp-calendar/server:${{ github.sha }}

      - name: Deploy to Cloud Run
        run: |
          gcloud run deploy mcp-calendar \
            --image=asia-northeast1-docker.pkg.dev/mcp-calendar-prod/mcp-calendar/server:${{ github.sha }} \
            --region=asia-northeast1
```

## 参考リンク

- Cloud Run ドキュメント: https://cloud.google.com/run/docs
- Cloud Storage クライアント: https://cloud.google.com/storage/docs/reference/libraries
- Firestore Rust SDK: https://github.com/google-apis-rs/google-cloud-rust
