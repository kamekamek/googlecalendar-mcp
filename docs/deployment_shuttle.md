# Shuttle.dev へのデプロイガイド（2025年・簡易構成）

Shuttle の無料枠では永続ファイルストレージが利用できません。今回は **トークンなどの機密情報はすべてメモリ内のみ** に保持し、サーバーの再起動ごとに再認証が必要になる構成でデプロイします。長期運用で永続化が必要になった場合は、PostgreSQL などの外部ストアへ移行してください。

## 0. 前提条件

- Shuttle アカウント（無料）と最新の CLI（`cargo install cargo-shuttle`）
- このリポジトリに含まれる `rust-toolchain.toml` により Rust nightly が自動で選択されます
- Google OAuth クライアント（Web アプリ型）を作成済みで、後述のリダイレクト URI を登録できること

## 1. プロジェクト側の準備

### 1.1 Cargo フィーチャー

`Cargo.toml` には既に Shuttle 関連の依存と `shuttle` フィーチャーが定義されています。Shuttle 上ではデフォルトフィーチャーが無効になるため、ローカル検証時も以下のようにフラグ付きで起動すると挙動を合わせられます。

```bash
cargo run --features shuttle
```

### 1.2 Shuttle.toml の作成

リポジトリルートに `Shuttle.toml` を配置します。`build.features` に `shuttle` を指定しておくと、デプロイ時に自動的に有効化されます。

```toml
name = "mcp-google-calendar"

[build]
features = ["shuttle"]

[deploy]
include = ["config/*.toml"]
```

### 1.3 Secrets の管理

ルートに `Secrets.toml`（本番用）と `Secrets.dev.toml`（ローカルテスト用）を作成し、`.gitignore` に追加します。今回の構成ではトークンをメモリのみで扱うため、**必ず `APP__SECURITY__USE_IN_MEMORY` を `true` に設定**してください。

```toml
OAUTH_CLIENT_ID = "your-google-client-id"
OAUTH_CLIENT_SECRET = "your-google-client-secret"
OAUTH_REDIRECT_URI = "https://<project>.shuttle.app/proxy/oauth/callback"
PROXY_ENABLED = "true"
SECURITY__USE_IN_MEMORY = "true"   # Shuttle では永続ストレージを使用しない
SERVER__BIND_ADDRESS = "0.0.0.0:8000"   # Shuttle の既定ポートに合わせる
SERVER__PUBLIC_URL = "https://<project>.shuttle.app"
```

> Shuttle が Secrets を環境変数に展開するとき、`APP__` プレフィックスは不要です。`SECURITY__USE_IN_MEMORY` は `APP__SECURITY__USE_IN_MEMORY` に対応します。

Secrets を CLI から書き込む場合は `shuttle secrets set KEY=VALUE` を利用します。`Shuttle.toml` と同様にリポジトリ外へ漏れないよう注意してください。

## 2. Shuttle 専用エントリーポイント

ローカル実行 (`src/main.rs`) と同等のサーバー処理を Shuttle 向けに再利用するため、`src/bin/shuttle.rs` を作成します。Shuttle ランタイムは `Router` を返せばポートの割り当てやライフサイクルを処理してくれるため、`TcpListener` を明示的に開く必要はありません。

```rust
// src/bin/shuttle.rs
use std::sync::Arc;

use axum::{Extension, Router};
use mcp_google_calendar::{
    config::AppConfig,
    handlers::build_router,
    mcp::service_factory,
    oauth::storage::{InMemoryTokenStorage, TokenStorage},
    AppState,
};
use rmcp::transport::sse_server::{SseServer, SseServerConfig};
use shuttle_runtime::SecretStore;
use tokio_util::sync::CancellationToken;

#[shuttle_runtime::main]
async fn shuttle(
    #[shuttle_runtime::Secrets] secrets: SecretStore,
) -> shuttle_axum::ShuttleAxum {
    // Secrets を環境変数に注入（AppConfig::load が `.env` と同様に読み込む）
    if let Some(client_id) = secrets.get("OAUTH_CLIENT_ID") {
        std::env::set_var("APP__OAUTH__CLIENT_ID", client_id);
    }
    if let Some(client_secret) = secrets.get("OAUTH_CLIENT_SECRET") {
        std::env::set_var("APP__OAUTH__CLIENT_SECRET", client_secret);
    }
    if let Some(redirect_uri) = secrets.get("OAUTH_REDIRECT_URI") {
        std::env::set_var("APP__OAUTH__REDIRECT_URI", redirect_uri);
    }
    std::env::set_var(
        "APP__PROXY__ENABLED",
        secrets
            .get("PROXY_ENABLED")
            .unwrap_or_else(|| "true".to_owned()),
    );
    std::env::set_var(
        "APP__SECURITY__USE_IN_MEMORY",
        secrets
            .get("SECURITY__USE_IN_MEMORY")
            .unwrap_or_else(|| "true".to_owned()),
    );
    std::env::set_var(
        "APP__SERVER__BIND_ADDRESS",
        secrets
            .get("SERVER__BIND_ADDRESS")
            .unwrap_or_else(|| "0.0.0.0:8000".to_owned()),
    );
    if let Some(public_url) = secrets.get("SERVER__PUBLIC_URL") {
        std::env::set_var("APP__SERVER__PUBLIC_URL", public_url);
    }

    // 設定読み込み（Shuttle 環境では use_in_memory=true が前提）
    let config = AppConfig::load().expect("load app config");
    let storage: Arc<dyn TokenStorage> = Arc::new(InMemoryTokenStorage::new());
    let state = Arc::new(AppState::new(config, storage).expect("initialize app state"));

    // SSE サーバーを組み込んだ Router を構築
    let bind_address = state
        .config
        .server
        .bind_address
        .parse()
        .expect("invalid bind address");
    let sse_config = SseServerConfig {
        bind: bind_address,
        sse_path: "/".into(),
        post_path: "/message".into(),
        ct: CancellationToken::new(),
        sse_keep_alive: None,
    };
    let (sse_server, sse_router) = SseServer::new(sse_config);
    let sse_token = sse_server.with_service(service_factory(state.clone()));

    let router: Router = build_router(state.clone())
        .nest("/mcp", sse_router)
        .layer(Extension(sse_token));

    Ok(router.into())
}
```

ポイント

- `APP__SECURITY__USE_IN_MEMORY=true` が指定されていれば `FileTokenStorage` は使われません。
- Secrets に設定した `SERVER__BIND_ADDRESS` を `0.0.0.0:8000` にしておくことで、Shuttle のロードバランサと整合します。
- トークンはプロセスが再起動すると消えるため、長時間利用では定期的な再認証が必要です。

## 3. デプロイ手順

1. プロジェクトを Shuttle と紐づけ  
   ```bash
   shuttle project create --name mcp-google-calendar   # 初回のみ
   shuttle project link --name mcp-google-calendar
   ```
2. Secrets のアップロード  
   ```bash
   shuttle deploy --secrets Secrets.toml
   # Makefile を利用する場合: make shuttle-deploy-secrets
   ```
3. Deploy 実行（`Shuttle.toml` の設定に従い `--features shuttle` でビルド）  
   ```bash
   shuttle deploy
   # Makefile を利用する場合: make shuttle-deploy
   ```
4. ログ確認  
   ```bash
   shuttle logs --latest
   # Makefile を利用する場合: make shuttle-logs
   ```

## 4. デプロイ後の確認

- `https://<project>.shuttle.app/health` が 200 を返すか確認する
- `.mcp.json` などクライアント設定の URL を `https://<project>.shuttle.app/mcp` に更新する
- 初回アクセスで OAuth 認証が求められ、再起動後は再認証が必要になる点を周知する

## 5. 制約と運用メモ

- **トークンが揮発**: プロセス再起動、デプロイ、スケールインなどでトークンは消えます。必要に応じて手動で再認証する運用を組み込んでください。
- **Secrets の更新**: 値を変更した場合は `shuttle deploy --secrets Secrets.toml` を再実行するか、`shuttle secrets set` で個別に書き換えます。
- **ログ・証跡**: Shuttle の Community Tier では常時稼働保証が無い点に注意し、必要なら自前で監視・Ping を行ってください。

将来的に永続化が必要になった場合は、`sqlx` と Shuttle Shared Postgres を有効化し、`FileTokenStorage` から DB バックエンドへ置き換える方針を検討してください。
