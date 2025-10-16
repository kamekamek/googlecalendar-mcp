# Codex GitHub Actions Integration

このドキュメントでは、Google Calendar MCPプロジェクトにおけるOpenAI Codexの統合について説明します。

## 概要

このリポジトリは、AI支援開発のために2つのアプローチを統合しています:

| ツール | 役割 | トリガー | 用途 |
|--------|------|----------|------|
| **Claude Code** | インタラクティブアシスタント | `@claude` メンション | コードレビュー、対話的な問題解決、リファクタリング |
| **Codex** | 自動CI/CD分析 | Push / PR作成時 | ビルドエラー分析、テスト失敗の診断、clippy警告の解析 |

## セットアップ

### 1. OpenAI APIキーの設定

Codex統合を有効にするには、リポジトリシークレットに `OPENAI_API_KEY` を追加する必要があります:

1. [OpenAI API Keys](https://platform.openai.com/api-keys) ページでAPIキーを生成
2. GitHubリポジトリの **Settings** → **Secrets and variables** → **Actions** に移動
3. **New repository secret** をクリック
4. 名前: `OPENAI_API_KEY`
5. 値: 生成したAPIキー
6. **Add secret** をクリック

### 2. 動作確認

APIキーを設定後、以下の操作でCodexが動作します:

```bash
# メインブランチへのプッシュ
git push origin main

# または、プルリクエストの作成
gh pr create --title "Test Codex Integration" --body "Testing automated analysis"
```

## ワークフローの詳細

### `.github/workflows/codex-ci.yml`

このワークフローは2つのジョブで構成されています:

#### Job 1: `rust-ci-with-codex`

Rustプロジェクトの標準的なCI/CDチェックを実行:

- **テスト実行**: `cargo +nightly test --all-features`
- **Clippy lint**: `cargo +nightly clippy --all-targets --all-features -- -D warnings`
- **フォーマットチェック**: `cargo +nightly fmt -- --check`

すべてのステップは `continue-on-error: true` で実行され、出力がアーティファクトとして保存されます。

#### Job 2: `codex-analysis`

エラーが検出された場合のみ実行され、Codexによる分析を行います:

- **入力**: CI/CDステップからの全出力
- **コンテキスト**: プロジェクト情報、Rust/Axum固有の情報
- **出力**: 根本原因分析、修正提案、コード例

### 安全性設定

```yaml
safety-strategy: drop-sudo
sandbox: read-only
```

- `drop-sudo`: ランナー上でCodexにスーパーユーザー権限を与えない(最も安全)
- `read-only`: Codexはファイルを読み取れるが、変更はできない

## 使用例

### シナリオ 1: テスト失敗

```bash
# 変更をコミット&プッシュ
git add src/main.rs
git commit -m "fix: update authentication logic"
git push origin feature-branch
```

テストが失敗した場合:
1. `rust-ci-with-codex` ジョブがテストエラーを検出
2. `codex-analysis` ジョブが自動的に起動
3. Codexが失敗の原因を分析し、修正案を提示
4. 結果がGitHub Actions Summaryに表示

### シナリオ 2: Clippy警告

Clippy警告が出た場合、Codexは:
- 警告の理由を説明
- 具体的なコード修正を提案
- Rustのベストプラクティスを推奨

### シナリオ 3: Edition 2024固有の問題

Rust nightly Edition 2024の互換性問題が発生した場合:
- Codexがnightly固有の機能変更を識別
- 新しいEditionに準拠したコードを提案
- 関連するドキュメントへの参照を提供

## Claudeとの使い分け

### Codexを使う場面:
- ✅ 自動的なCI/CDエラー診断が必要
- ✅ ビルド失敗の迅速な原因特定
- ✅ 繰り返し発生するlint警告の分析

### Claudeを使う場面:
- ✅ コードレビューと詳細なフィードバック
- ✅ アーキテクチャの議論と設計相談
- ✅ 複雑なリファクタリング作業
- ✅ インタラクティブなデバッグセッション

**使い方:**
```bash
# Issue/PRコメントで@claudeをメンション
@claude このエラーの解決方法を教えて
```

## カスタマイズ

### モデルの変更

デフォルトは `gpt-4-turbo` ですが、変更可能:

```yaml
model: gpt-4o  # より高速な応答
# または
model: o1-preview  # より深い推論
```

### プロンプトのカスタマイズ

`.github/workflows/codex-ci.yml` の `prompt` セクションを編集:

```yaml
prompt: |
  あなたはRustの専門家です。
  [カスタムプロンプト]
```

### トリガーの調整

特定のファイルのみを対象にする:

```yaml
on:
  push:
    branches: [main]
    paths:
      - 'src/**/*.rs'
      - 'Cargo.toml'
  pull_request:
    paths:
      - 'src/**/*.rs'
```

## コスト管理

OpenAI API使用料金を管理するために:

### 推定コスト
- **GPT-4 Turbo**: 約$0.01-0.10 / CI実行
- **トークン数**: 平均 1,000-10,000トークン / 実行
- **月間推定**: 50PR × $0.05 = **$2.50/月**

### コスト削減の工夫:

1. **条件付き実行**: エラー時のみCodexを起動(現在の設定)
   ```yaml
   if: needs.rust-ci-with-codex.outputs.has_errors == 'true'
   ```

2. **ブランチ制限**: メインブランチとPRのみ
   ```yaml
   on:
     push:
       branches: [main]
     pull_request:
   ```

3. **モデル選択**: 小さなモデルを使用
   ```yaml
   model: gpt-3.5-turbo  # より安価
   ```

## トラブルシューティング

### 問題: Codexジョブがスキップされる

**原因**: `OPENAI_API_KEY` が設定されていない

**解決策**:
```bash
# GitHub CLIでシークレットを設定
gh secret set OPENAI_API_KEY
# プロンプトでAPIキーを入力
```

### 問題: "Resource not accessible by integration" エラー

**原因**: ワークフロー権限が不足

**解決策**: `.github/workflows/codex-ci.yml` の `permissions` を確認

### 問題: Codexの分析が不正確

**原因**: コンテキストが不足している

**解決策**: `prompt` にプロジェクト固有の情報を追加:
```yaml
prompt: |
  IMPORTANT PROJECT NOTES:
  - 認証にはGoogle OAuth 2.0 + PKCEフローを使用
  - トークンはconfig/tokens.jsonに保存
  - すべてのエンドポイントはuser_idパラメータが必須

  [既存のプロンプト]
```

## 参考リンク

- [openai/codex-action GitHub](https://github.com/openai/codex-action)
- [OpenAI API Documentation](https://platform.openai.com/docs)
- [Codex Security Best Practices](https://github.com/openai/codex-action/blob/main/docs/security.md)
- [Claude Code Action](https://github.com/anthropics/claude-code-action)

## 今後の拡張案

- [ ] 自動修正のプルリクエスト作成
- [ ] Codexによるテストコード生成
- [ ] セキュリティ脆弱性スキャンとの統合
- [ ] パフォーマンスベンチマークの自動分析
