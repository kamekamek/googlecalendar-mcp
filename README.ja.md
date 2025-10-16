# Google Calendar MCP Server

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-nightly-orange.svg)](https://www.rust-lang.org/)
[![MCP](https://img.shields.io/badge/MCP-0.8.1-green.svg)](https://modelcontextprotocol.io/)

**[English](README.md) | [日本語](README.ja.md)**

AIエージェントがGoogle Calendarの予定を読み書きできるようにするAxumベースのMCPサーバーです。Claude Codeなどのツールから、OAuth認証を通じてGoogle Calendar APIにアクセスできます。

## 特徴

- 🔐 PKCE対応のOAuth 2.0認証
- 📅 4つの操作: 予定の一覧取得・取得・作成・更新
- 🚀 Server-Sent Events (SSE) によるリモートMCPトランスポート
- 🔄 自動トークンリフレッシュ
- 👥 マルチユーザー対応（ユーザーごとにトークンを分離）
- 🛡️ セキュリティ優先: 削除機能は意図的に無効化
- 🔌 Claude Code完全対応

## セットアップガイド

### 1. Google Cloud プロジェクトの設定

#### 1-1. プロジェクトの作成

1. [Google Cloud Console](https://console.cloud.google.com/) にアクセス
2. 画面上部のプロジェクト選択ドロップダウンをクリック
3. 「新しいプロジェクト」をクリック
4. プロジェクト名を入力（例: `mcp-calendar-server`）して「作成」

#### 1-2. Google Calendar API の有効化

1. サイドメニューから「APIとサービス」→「ライブラリ」を選択
2. 検索ボックスに「Google Calendar API」と入力
3. 「Google Calendar API」をクリック
4. 「有効にする」ボタンをクリック
5. 「API は有効です」と表示されたら完了

#### 1-3. OAuth 同意画面の設定

1. 「APIとサービス」→「OAuth 同意画面」を選択
2. **User Type**: 「外部」を選択して「作成」
   - Google Workspaceで社内のみの場合は「内部」でも可
3. **アプリ情報**:
   - アプリ名: `MCP Calendar Server`（任意）
   - ユーザーサポートメール: 自分のメールアドレスを選択
   - デベロッパーの連絡先情報: 自分のメールアドレスを入力
4. 「保存して次へ」をクリック

#### 1-4. スコープの追加

1. 「スコープを追加または削除」をクリック
2. フィルタに `calendar` と入力
3. `https://www.googleapis.com/auth/calendar` にチェック
4. 「更新」→「保存して次へ」

#### 1-5. テストユーザーの追加

1. 「テストユーザー」セクションで「ADD USERS」をクリック
2. 自分のGoogleアカウントのメールアドレスを入力
3. 「追加」→「保存して次へ」
4. 「ダッシュボードに戻る」をクリック

> **重要**: テストモードでは、ここで追加したユーザーのみがログインできます。

#### 1-6. OAuth 認証情報の作成

1. 「APIとサービス」→「認証情報」を選択
2. 「認証情報を作成」→「OAuth クライアント ID」をクリック
3. **アプリケーションの種類**: 「ウェブアプリケーション」を選択
4. **名前**: `MCP Calendar OAuth Client`（任意）
5. **承認済みのリダイレクト URI** で「URI を追加」をクリックし、以下を追加:
   - `http://localhost:8080/oauth/callback`
   - `https://localhost:8443/proxy/oauth/callback`
6. 「作成」をクリック
7. 表示された「クライアントID」と「クライアントシークレット」をコピーして保存
   - ❗この情報は後で使うのでメモしておいてください

### 2. ローカル環境のセットアップ

#### 2-1. リポジトリのクローンとRustのインストール

```bash
# リポジトリをクローン
git clone https://github.com/kamekamek/mcp-google-calendar.git
cd mcp-google-calendar

# Rust nightly をインストール（まだの場合）
rustup toolchain install nightly
```

#### 2-2. 環境変数の設定

```bash
# .env.example を .env にコピー
cp .env.example .env
```

`.env` ファイルを編集して、先ほど取得したGoogle OAuth認証情報を設定:

```env
APP__OAUTH__CLIENT_ID="<クライアントIDをここに貼り付け>"
APP__OAUTH__CLIENT_SECRET="<クライアントシークレットをここに貼り付け>"
APP__SERVER__PUBLIC_URL="https://localhost:8443"
APP__PROXY__ENABLED=true
```

### 3. Caddyのインストールと起動

#### 3-1. mkcertで証明書を生成

```bash
# mkcert をインストール（Homebrewの場合）
brew install mkcert

# ローカルCAをインストール
mkcert -install

# localhost用の証明書を生成
mkcert localhost 127.0.0.1 ::1
# → localhost+2.pem と localhost+2-key.pem が作成されます
```

#### 3-2. Caddyのインストールと起動

```bash
# Caddyをインストール
brew install caddy

# Caddyを起動（別のターミナルで実行）
caddy run --config caddyfile
```

このターミナルはCaddyが動作し続けるので、開いたままにしておきます。

#### 3-3. MCPサーバーの起動

新しいターミナルを開いて:

```bash
cd mcp-google-calendar
cargo +nightly run
```

サーバーが `127.0.0.1:8080` で起動します。このターミナルも開いたままにしておきます。

### 4. Claude Codeでの設定

#### 4-1. .mcp.json の設定

`.mcp.json` ファイルを編集（存在しない場合は作成）:

```json
{
  "mcpServers": {
    "google_calendar": {
      "type": "sse",
      "url": "https://localhost:8443/mcp",
      "metadata": {
        "description": "Google Calendar MCP Server"
      }
    }
  }
}
```

#### 4-2. Claude Codeを起動

```bash
# Claude Code CLIを起動
claude
```

起動後、以下のコマンドを実行:

```
/mcp
```

MCP接続のメニューが表示されます。

#### 4-3. 認証フロー

1. MCPサーバー一覧から `google_calendar` を選択
2. 「Authenticate」ボタンをクリック
3. ブラウザが自動的に開き、Google OAuth認証画面が表示されます
4. テストユーザーとして登録したGoogleアカウントでログイン
5. アプリの権限を確認して「許可」をクリック
6. ブラウザに「認証完了」と表示されたらClaude Codeに戻る
7. 接続が完了すると、利用可能なツール一覧が表示されます

### 5. 動作確認

Claude Codeで以下を試してみましょう:

```
今週の予定を教えて
```


## 利用可能なツール

すべてのツールで `user_id` パラメータが必要です（Claude Codeが自動的に設定します）。

### google_calendar_list_events
予定の一覧を取得します。

**パラメータ:**
- `time_min`: 開始日時フィルタ（RFC3339形式: `2025-10-20T00:00:00+09:00`）
- `time_max`: 終了日時フィルタ
- `max_results`: 最大取得件数（1-2500）
- `calendar_id`: カレンダーID（省略時は "primary"）

### google_calendar_get_event
IDを指定して予定を1件取得します。

**パラメータ:**
- `event_id`: 予定のID（必須）
- `calendar_id`: カレンダーID（省略時は "primary"）

### google_calendar_create_event
新しい予定を作成します。

**パラメータ:**
- `summary`: 予定のタイトル（必須）
- `start`: 開始日時（必須）
- `end`: 終了日時（必須）
- `description`: 説明（任意）
- `location`: 場所（任意）

**開始・終了日時の形式:**
```
"2025-10-20T10:00:00+09:00"
```

### google_calendar_update_event
既存の予定を更新します。

**パラメータ:**
- `event_id`: 予定のID（必須）
- `summary`, `start`, `end`, `description`, `location`: 更新したい項目（任意）

## トラブルシューティング

### 認証エラーが出る

**原因**: テストユーザーに追加されていないGoogleアカウントでログインしようとしている

**解決方法**:
1. Google Cloud Console → OAuth同意画面 → テストユーザー
2. 使用するGoogleアカウントを追加

### トークンリフレッシュエラー

**原因**: リフレッシュトークンは初回認証時のみ発行されます

**解決方法**:
1. https://myaccount.google.com/permissions にアクセス
2. 「MCP Calendar Server」を探して削除
3. Claude Codeで再度認証

### HTTPSエラー

**原因**: 証明書がない、またはCaddyが起動していない

**解決方法**:
```bash
# 証明書の確認
ls localhost+2*.pem
# → localhost+2.pem と localhost+2-key.pem が存在することを確認

# Caddyが起動しているか確認
lsof -i :8443
# → caddyのプロセスが表示されればOK
```

### EventDateTime形式エラー

RFC3339形式を使用してください:
```
"2025-10-20T10:00:00+09:00"
```

または、オブジェクト形式:
```json
{
  "dateTime": "2025-10-20T10:00:00+09:00",
  "timeZone": "Asia/Tokyo"
}
```

## 開発者向け

### ビルドとテスト

```bash
# フォーマット
cargo +nightly fmt

# リント
cargo +nightly clippy -- -D warnings

# テスト
cargo +nightly test
```

### 設定ファイル

完全な設定オプションは `config/config.toml` を参照してください。

環境変数で設定を上書き可能:
- `APP__OAUTH__CLIENT_ID`
- `APP__OAUTH__CLIENT_SECRET`
- `APP__SERVER__PUBLIC_URL`
- `APP__SECURITY__USE_IN_MEMORY` (true/false)
- `APP__PROXY__ENABLED` (true/false)

## ライセンス

MIT License - 詳細は [LICENSE](LICENSE) を参照

## リンク

- [Model Context Protocol](https://modelcontextprotocol.io/)
- [Google Calendar API](https://developers.google.com/calendar/api)
- [Issue Tracker](https://github.com/kamekamek/mcp-google-calendar/issues)
