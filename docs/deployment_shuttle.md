# Shuttle.dev へのデプロイガイド

Shuttle.dev は Rust 専用のホスティングプラットフォームで、`cargo shuttle deploy` だけでデプロイが完了します。

## 前提条件

- Shuttle アカウント（無料）: https://www.shuttle.dev/
- Shuttle CLI のインストール

```bash
cargo install cargo-shuttle
```

## 1. Shuttle.toml の作成

プロジェクトルートに `Shuttle.toml` を作成：

```toml
name = "mcp-google-calendar"
```

## 2. main.rs の調整

Shuttle は `#[shuttle_runtime::main]` アトリビュートを使います。

`src/main.rs` を以下のように修正：

```rust
use mcp_google_calendar::*;
use std::sync::Arc;

#[shuttle_runtime::main]
async fn main(
    #[shuttle_runtime::Secrets] secrets: shuttle_runtime::SecretStore,
) -> shuttle_axum::ShuttleAxum {
    // 環境変数を Shuttle Secrets から取得
    std::env::set_var("APP__OAUTH__CLIENT_ID", secrets.get("OAUTH_CLIENT_ID").unwrap());
    std::env::set_var("APP__OAUTH__CLIENT_SECRET", secrets.get("OAUTH_CLIENT_SECRET").unwrap());
    std::env::set_var("APP__OAUTH__REDIRECT_URI", secrets.get("OAUTH_REDIRECT_URI").unwrap());
    std::env::set_var("APP__PROXY__ENABLED", "true");

    // 設定読み込み
    let config = config::AppConfig::load().expect("config load");

    // トークンストレージ（本番環境では Shuttle Storage を検討）
    let storage = if config.security.use_in_memory {
        Arc::new(oauth::storage::InMemoryTokenStorage::new()) as Arc<dyn oauth::storage::TokenStorage>
    } else {
        Arc::new(oauth::storage::FileTokenStorage::new(&config.security.token_store_path))
            as Arc<dyn oauth::storage::TokenStorage>
    };

    let state = Arc::new(AppState::new(config, storage)?);
    let app = handlers::create_router(state);

    Ok(app.into())
}
```

## 3. Cargo.toml に依存を追加

```toml
[dependencies]
# 既存の依存関係...
shuttle-runtime = "0.48.0"
shuttle-axum = "0.48.0"
```

## 4. Secrets の設定

```bash
# Shuttle にログイン
cargo shuttle login

# Secrets を設定
cargo shuttle secrets set OAUTH_CLIENT_ID="<Google OAuth クライアント ID>"
cargo shuttle secrets set OAUTH_CLIENT_SECRET="<Google OAuth クライアントシークレット>"
cargo shuttle secrets set OAUTH_REDIRECT_URI="https://<your-app>.shuttle.app/proxy/oauth/callback"
```

## 5. Google Cloud Console の設定

OAuth クライアント ID のリダイレクト URI に以下を追加：

```
https://<your-app-name>.shuttle.app/proxy/oauth/callback
```

> **注意**: アプリ名は `cargo shuttle project new` または初回デプロイ時に指定します。

## 6. デプロイ

```bash
# 初回デプロイ（プロジェクト作成）
cargo shuttle project new

# デプロイ実行
cargo shuttle deploy

# ログ確認
cargo shuttle logs
```

## 7. Claude Code で接続

`.mcp.json` に以下を追加：

```json
{
  "mcpServers": {
    "google_calendar": {
      "type": "sse",
      "url": "https://<your-app-name>.shuttle.app/mcp"
    }
  }
}
```

## トラブルシューティング

### ビルドエラー

Shuttle は Edition 2021 をサポートしています。`rmcp` が Edition 2024 を要求する場合は、以下を検討：

1. `rmcp` のバージョンを確認
2. Shuttle の Rust バージョンを nightly に変更（`rust-toolchain.toml` は認識されない可能性）

### トークン永続化

`config/tokens.json` はデプロイごとにリセットされます。本番環境では以下を検討：

- Shuttle Storage: https://docs.shuttle.dev/resources/shuttle-persist
- 外部データベース（Shuttle Postgres など）
- Cloud Storage（Google Cloud Storage）

```rust
#[shuttle_runtime::main]
async fn main(
    #[shuttle_runtime::Secrets] secrets: shuttle_runtime::SecretStore,
    #[shuttle_runtime::Persist] persist: shuttle_persist::PersistInstance,
) -> shuttle_axum::ShuttleAxum {
    // persist を使ってトークンを永続化
}
```

## 料金

- **無料枠**: 1 プロジェクト、カスタムドメイン 1 つ、Starter DB
- **従量課金**: ビルド $0.025/分、ストレージ $0.12/GB/月、ネットワーク $0.10/GB

個人利用なら無料枠で十分です。

## 参考リンク

- Shuttle ドキュメント: https://docs.shuttle.dev/
- Shuttle Discord: https://discord.gg/shuttle
