# MCP サーバー構築の学習メモ

## 1. MCP サーバーの基本
- Rust nightly が必須 (`rmcp` が Edition 2024 を要求)。`rustup toolchain install nightly` と `rust-toolchain.toml` の用意を忘れない。
- `.env` はリポジトリ直下に置く。`dotenvy` はカレントディレクトリの `.env` のみ読み込む。
- SSE (`/mcp` + `/mcp/message`) と HTTP シム (`/mcp/tool`) を実装しておくと、動作確認がしやすい。

## 2. Google OAuth 認証
- `user_id` ごとにトークンを保存。`GET /oauth/authorize?user_id=demo` → ブラウザで同意 → `/oauth/callback` で `config/tokens.json` に保存されているか確認。
- `.env` に実際の `APP__OAUTH__CLIENT_ID` / `APP__OAUTH__CLIENT_SECRET` を設定しないと 400/401 エラーが発生する。
- Google Cloud Console には `http://localhost:8080/oauth/callback` と、プロキシ用の URI (`http://localhost:8080/proxy/oauth/callback` or HTTPS ドメイン) を登録。

## 3. SSE と HTTP Stream
- **SSE**: 実装が簡単で対応クライアントが多いが、双方向通信は不得意。テキストベース。
- **HTTP Stream**: 1コネクションでフルデュプレックス。HTTP/2 やリバースプロキシ設定が必要。Claude Code などは HTTP Stream を推奨する流れ。

## 4. Claude Code と Dynamic Client Registration (DCR)
- Claude Code は OAuth 2.1 + DCR を必須とする。Google OAuth は DCR を提供しないため、そのままでは "Incompatible auth server" エラーになる。
- 解決策:
  - Claude Desktop のカスタムコネクタを利用 (固定 Client ID/Secret を直接入力)。
  - DCR 対応プロキシを構築 (`mcp-front` など)。
  - STDIO MCP サーバーを使用し、OAuth をサーバー内で完結させる。

## 5. DCR 対応プロキシの実装ポイント
- `/proxy/oauth/register`・`/proxy/oauth/authorize`・`/proxy/oauth/callback`・`/proxy/oauth/token` などのエンドポイントを提供し、内部で Google OAuth と橋渡しする。
- Google から受け取った `code` をプロキシ用コードに変換し、クライアントはプロキシ経由でトークン交換を行う。
- HTTPS 必須。ローカルでは mkcert + Caddy/Nginx で `https://localhost:8443` を用意。`server.public_url` も HTTPS に合わせる。
- `config/config.toml` で `proxy.enabled = true` を設定し、Google 側のリダイレクト URI も `/proxy/oauth/callback` に合わせる。

## 6. プロキシを介した後の課題
- Claude Code が取得したアクセストークンを MCP サーバーのトークンストレージへ取り込む処理が未実装のままだと、`user_id`/トークンが空で 401 エラーになる。
- 対応策: `Authorization: Bearer ...` ヘッダを受け取ってサーバー側に保存するロジックを追加し、必要であれば Google の `/userinfo` や ID トークンからユーザーIDを判定する。
- 暫定的には MCP Inspector や STDIO サーバーで動作確認を継続するか、Claude Desktop のコネクタを利用する。

## 7. HTTPS プロキシ (Caddy) の設定例
```
https://localhost:8443 {
    tls /path/to/localhost.pem /path/to/localhost-key.pem
    reverse_proxy http://127.0.0.1:8080
}
```
`mkcert -install` で自己署名証明書を OS に登録しておくと、ブラウザや Claude Code でも警告が出にくい。

## 8. トラブルシュートで得た知見
- `client sent an HTTP request to an HTTPS server` → Claude Code の URL が `http://` になっている、またはプロキシが HTTPS を待ち受けているのに平文でアクセスしている。
- `Protected resource ... does not match expected ...` → `server.public_url` の値とプロキシ URL が一致していない。
- `Dynamic client registration failed` → プロキシを挟まないと解決不可。
- `MCP error -32603 ... 404` → 認可したユーザーに該当カレンダーが存在しない、またはスコープ不足。
- `user '' is not authorized` → トークンが保存されていない。手動で `/oauth/authorize` → `/oauth/callback` を実行する。

## 10. 2025-10-14 の接続トラブルから得た教訓

### 10.1 SSE エンドポイントのパス
- Claude Code v2.0.14 は `https://localhost:8443/mcp` へ直接 GET `Accept: text/event-stream` を送り、404 が返ると再接続する。
- Axum 側の `SseServer` はデフォルトで `/sse` を公開するため、`SseServerConfig.sse_path = "/"` に変更し、`Router::nest("/mcp", sse_router)` と組み合わせて `/mcp` が有効になるよう修正した。
- ドキュメント上も SSE の URL を `/mcp/sse` ではなく `/mcp` に統一し、`.mcp.json`・Caddy 設定・クライアント側の設定が混在しないようにする。

### 10.2 Authorization ヘッダーの取り込み
- プロキシ経由で取得したアクセストークンは、MCP サーバーの `token_storage` に明示的に保存しない限り 401 (`user 'X' is not authorized`) が発生する。
- HTTP `/mcp/tool` と SSE 双方で `Authorization: Bearer ...` を検査し、ヘッダーから `TokenInfo` を構築する共通ヘルパー (`token_ingest::ingest_bearer_token_from_headers`) を導入した。付随メタデータは以下の優先度で扱う：
  - `x-mcp-oauth-refresh-token` / `x-oauth-refresh-token`
  - `x-mcp-oauth-scope` / `x-oauth-scope`
  - `x-mcp-oauth-expires-at` (RFC 3339 または Unix 秒) もしくは `x-mcp-oauth-expires-in`
  - トークンタイプ (`x-mcp-oauth-token-type`)
- 保存対象のストレージは設定に従って切り替わるため、`security.use_in_memory = true` の場合はプロセス終了と同時にトークンが消える。永続化したい場合は `use_in_memory = false` とし、`config/tokens.json` の権限を確認する。

### 10.3 プロキシの WARN ログの扱い
- Caddy の `http.handlers.reverse_proxy aborting with incomplete response` + `context canceled` は、SSE クライアントが自発的に切断／再接続した場合にも出る既定の WARN。単発であれば問題なし。
- 同時刻にサーバーログへ 401 などが出ている場合は、アプリ側のレスポンスが原因でセッションが落ちていないかを突き合わせる。

### 10.4 トークン保存の確認手順
1. サーバーを DEBUG ログで起動し、`stored bearer token from headers` もしくは `updated bearer token from headers` が出力されるか確認する。
2. 永続ストレージ利用時は `config/tokens.json` が作成され、`user_id` ごとにアクセストークンが保存されているかを確認する（機微情報なので閲覧後は削除/権限調整）。
3. 401 が続く場合は、クライアントが送信している `user_id` と Bearer トークンの所有者が一致しているか、ヘッダー名にタイプミスがないかを見直す。

### 10.5 再発防止チェックリスト
- `.mcp.json` や Caddy の `reverse_proxy` 先は常に HTTPS URL (`https://localhost:8443/mcp`) を指すようにする。
- サーバー起動時のログで `sse_path = "/mcp"` が出ているか確認する。
- プロキシ経由接続前に `GET /health` で Axum 側が応答するかチェックし、ネットワーク／証明書の問題を切り分ける。
- 手元の `.claude/debug/latest`（または対応ツールのデバッグログ）とサーバーの `latest.log` を突き合わせ、タイムスタンプを基準に原因を特定するクセを付ける。

## 9. ドキュメントの整備
- README、Usage Guide、Design Note などを日本語化し、手順・要件・構成図を整理すると迷いにくい。
- 特に Option B (DCR プロキシ) を利用する場合は「Google 側で追加すべきリダイレクト URI」「HTTPS が必須」「プロキシ経由で得たトークンをサーバーへ回収する処理が必要」という点を明記しておく。

## 次回の着手前チェックリスト
1. `.env` と夜間ツールチェーンが準備できているか。
2. Google Cloud Console のリダイレクト URI が最新の構成 (`/oauth/callback` + `/proxy/oauth/callback`) に一致しているか。
3. `server.public_url` がアクセス元 (HTTPS) に合わせて設定されているか。
4. HTTPS プロキシ (Caddy/Nginx) が正常に動作しているか。自己署名証明書または Let’s Encrypt を導入済みか。
5. Claude Code など DCR クライアントに対応する場合、アクセストークンをサーバーへ取り込むロジックの有無を確認。
6. ドキュメント更新を先に行ってから実装に入るとスムーズ。
