# リリース手順

## ブランチ戦略

- **`main`**: 安定ブランチ。常にリリース可能な状態を維持する
- 機能開発は feature ブランチから `main` への PR で行う

## タグ戦略

セマンティックバージョニングに従う: `v{major}.{minor}.{patch}`

- 正式リリース: `v0.1.0`, `v1.2.3`
- プレリリース: `v0.1.0-rc.1`, `v0.1.0-beta.1`

タグの push が release ワークフローをトリガーする。

## ビルドターゲット

| OS    | Architecture  | Target Triple                  |
|-------|---------------|--------------------------------|
| macOS | Apple Silicon | `aarch64-apple-darwin`         |
| macOS | Intel         | `x86_64-apple-darwin`          |
| Linux | x86_64        | `x86_64-unknown-linux-gnu`     |
| Linux | ARM64         | `aarch64-unknown-linux-gnu`    |

## リリース手順

1. `Cargo.toml` の `version` を更新する
2. `mise run check` で全チェックが通ることを確認する
3. コミットする
   ```bash
   git add Cargo.toml
   git commit -m "Bump version to x.y.z"
   ```
4. タグを作成して push する
   ```bash
   git tag v{x.y.z}
   git push origin main --tags
   ```
5. [GitHub Actions](../../actions) でリリースワークフローが完了することを確認する
6. [Releases](../../releases) ページで 4 つのアーティファクトが添付されていることを確認する

## インストール（mise 経由）

```bash
# 最新版
mise use -g github:tact-software/mdskim

# バージョン指定
mise use -g github:tact-software/mdskim@0.1.0
```

> **Note**: Mermaid レンダリング・数式・PDF エクスポート機能は Node.js パッケージが別途必要です。バイナリ配布はコアのビューア機能のみを含みます。
