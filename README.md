# Nostr MCP サーバー

Claude などの AI エージェントが Nostr ネットワークと対話するための Model Context Protocol (MCP) サーバーです。

---

## 概要

このサーバーは、AI アシスタントと Nostr 分散型ソーシャルネットワークの橋渡しを提供します。AI エージェントが以下の操作を行えるようになります：

- Nostr にノートを投稿
- タイムラインの取得（パーソナライズまたはグローバル）
- ノートの検索
- ユーザープロフィールの取得
- 長文記事 (NIP-23) の投稿・取得
- 記事の下書き保存・管理

## インストール

### 必要条件

- Rust（最新の安定版）
- Nostr 秘密鍵 (nsec) - 書き込みアクセスに必要（任意）

### ソースからビルド

```bash
git clone https://github.com/tami1A84/rust-nostr-mcp.git
cd rust-nostr-mcp
cargo build --release
```

バイナリは `target/release/nostr-mcp-server` に生成されます。

## 設定

### 設定ファイル（推奨）

サーバーは `~/.config/rust-nostr-mcp/config.json` の設定ファイルを使用します。これは [algia](https://github.com/mattn/algia) の規則に従っています。

**重要:** 秘密鍵はローカルに保存され、**AI エージェントには渡されません**。

```json
{
  "relays": {
    "wss://relay.damus.io": { "read": true, "write": true, "search": false },
    "wss://nos.lol": { "read": true, "write": true, "search": false },
    "wss://relay.nostr.band": { "read": true, "write": true, "search": true },
    "wss://nostr.wine": { "read": true, "write": false, "search": true },
    "wss://relay.snort.social": { "read": true, "write": true, "search": false }
  },
  "privatekey": "nsec1..."
}
```

**リレーオプション:**
- `read`: このリレーからイベントを取得
- `write`: このリレーにイベントを公開
- `search`: NIP-50 検索クエリに使用

初回起動時にデフォルト設定ファイルが自動的に作成されます。

### 環境変数（レガシー）

後方互換性のため、環境変数も使用できます：

```bash
# 書き込みアクセスに必要（ノート投稿用）
# NSEC または NOSTR_SECRET_KEY のいずれかを使用
NSEC=nsec1...

# または16進数形式
# NOSTR_SECRET_KEY=...

# オプション: カスタムリレーリスト（カンマ区切り）
NOSTR_RELAYS=wss://relay.damus.io,wss://nos.lol,wss://relay.nostr.band

# オプション: NIP-50 検索対応リレー（カンマ区切り）
NOSTR_SEARCH_RELAYS=wss://relay.nostr.band,wss://nostr.wine

# オプション: ログレベル
RUST_LOG=info
```

### デフォルトリレー

指定がない場合、以下のリレーを使用します：

**一般リレー:**
- `wss://relay.damus.io`
- `wss://nos.lol`
- `wss://relay.nostr.band`
- `wss://nostr.wine`
- `wss://relay.snort.social`

**検索リレー (NIP-50):**
- `wss://relay.nostr.band`
- `wss://nostr.wine`

## 使用方法

### Claude Desktop の設定

Claude Desktop の設定ファイルに以下を追加します：

**macOS:** `~/Library/Application Support/Claude/claude_desktop_config.json`

**Windows:** `%APPDATA%\Claude\claude_desktop_config.json`

**Linux:** `~/.config/Claude/claude_desktop_config.json`

```json
{
  "mcpServers": {
    "nostr": {
      "command": "/path/to/nostr-mcp-server"
    }
  }
}
```

**注意:** 秘密鍵は環境変数ではなく `~/.config/rust-nostr-mcp/config.json` で設定してください。これにより、鍵が AI エージェントに公開されることがなく、より安全です。

### Goose の設定

[Goose](https://github.com/block/goose) は Block 社が開発した AI コーディングアシスタントで、MCP サーバーをサポートしています。

Goose の設定ファイルに以下を追加します：

**場所:** `~/.config/goose/config.yaml`

```yaml
extensions:
  nostr:
    name: nostr
    type: stdio
    enabled: true
    cmd: /path/to/nostr-mcp-server
```

または CLI で設定：

```bash
goose configure
# "Add Extension" -> "Command-line Extension" を選択
# Name: nostr
# Command: /path/to/nostr-mcp-server
```

**注意:** 秘密鍵は環境変数ではなく `~/.config/rust-nostr-mcp/config.json` で設定してください。

### その他の MCP クライアント

このサーバーは MCP 互換のすべてのクライアントで動作します。JSON-RPC 2.0 over stdio で通信します。

### 読み取り専用モード

秘密鍵が設定されていない場合、サーバーは読み取り専用モードで起動します。以下の機能は使用可能です：
- タイムラインの取得（グローバル）
- ノートの検索
- プロフィールの取得
- 記事の取得

ただし、ノートの投稿、記事の投稿、下書きの保存はできません。

## 利用可能なツール

ツール名は [algia](https://github.com/mattn/algia) の規則に従い、`nostr_` プレフィックスを使用しています。

### 1. `post_nostr_note`

Nostr ネットワークにショートテキストノート (Kind 1) を投稿します。

**パラメータ:**
| 名前 | 型 | 必須 | 説明 |
|------|------|------|------|
| `content` | string | はい | 投稿するノートのテキスト内容 |

**例:**
```json
{
  "name": "post_nostr_note",
  "arguments": {
    "content": "こんにちは、Nostr！"
  }
}
```

### 2. `get_nostr_timeline`

タイムラインから最新のノートを取得します。認証済みの場合はフォロー中のユーザーのノート、それ以外はグローバルタイムラインを返します。

**パラメータ:**
| 名前 | 型 | 必須 | デフォルト | 説明 |
|------|------|------|------|------|
| `limit` | number | いいえ | 20 | 取得するノートの最大数 (1-100) |

**例:**
```json
{
  "name": "get_nostr_timeline",
  "arguments": {
    "limit": 10
  }
}
```

### 3. `search_nostr_notes`

NIP-50 対応リレーを使用してキーワードでノートを検索します。

**パラメータ:**
| 名前 | 型 | 必須 | デフォルト | 説明 |
|------|------|------|------|------|
| `query` | string | はい | - | 検索クエリ文字列 |
| `limit` | number | いいえ | 20 | 結果の最大数 (1-100) |

**例:**
```json
{
  "name": "search_nostr_notes",
  "arguments": {
    "query": "ビットコイン",
    "limit": 15
  }
}
```

### 4. `get_nostr_profile`

公開鍵で Nostr ユーザーのプロフィール情報を取得します。

**パラメータ:**
| 名前 | 型 | 必須 | 説明 |
|------|------|------|------|
| `pubkey` | string | はい | npub (bech32) または16進数形式の公開鍵 |

**例:**
```json
{
  "name": "get_nostr_profile",
  "arguments": {
    "pubkey": "npub1..."
  }
}
```

### 5. `post_nostr_article`

Nostr ネットワークに長文記事 (Kind 30023, NIP-23) を投稿します。

**パラメータ:**
| 名前 | 型 | 必須 | 説明 |
|------|------|------|------|
| `title` | string | はい | 記事のタイトル |
| `content` | string | はい | Markdown 形式の記事本文 |
| `summary` | string | いいえ | 記事の要約 |
| `image` | string | いいえ | ヘッダー画像の URL |
| `tags` | array | いいえ | トピックハッシュタグ |
| `published_at` | number | いいえ | 公開日時の Unix タイムスタンプ |
| `identifier` | string | いいえ | 記事の識別子（d タグ） |

**例:**
```json
{
  "name": "post_nostr_article",
  "arguments": {
    "title": "Nostr プロトコル入門",
    "content": "# はじめに\n\nNostr は分散型ソーシャルプロトコルです...",
    "summary": "Nostr プロトコルの基本的な仕組みを解説します",
    "tags": ["nostr", "protocol", "入門"]
  }
}
```

### 6. `get_nostr_articles`

Nostr ネットワークから長文記事 (Kind 30023) を取得します。

**パラメータ:**
| 名前 | 型 | 必須 | デフォルト | 説明 |
|------|------|------|------|------|
| `author` | string | いいえ | - | 著者の公開鍵でフィルタ |
| `tags` | array | いいえ | - | ハッシュタグでフィルタ |
| `limit` | number | いいえ | 20 | 取得する記事の最大数 (1-100) |

**例:**
```json
{
  "name": "get_nostr_articles",
  "arguments": {
    "tags": ["bitcoin"],
    "limit": 5
  }
}
```

### 7. `save_nostr_draft`

記事を下書き (Kind 30024) として保存します。

**パラメータ:**
| 名前 | 型 | 必須 | 説明 |
|------|------|------|------|
| `title` | string | はい | 記事のタイトル |
| `content` | string | はい | Markdown 形式の記事本文 |
| `summary` | string | いいえ | 記事の要約 |
| `image` | string | いいえ | ヘッダー画像の URL |
| `tags` | array | いいえ | トピックハッシュタグ |
| `identifier` | string | いいえ | 記事の識別子（d タグ） |

**例:**
```json
{
  "name": "save_nostr_draft",
  "arguments": {
    "title": "執筆中の記事",
    "content": "# 下書き\n\nまだ完成していない記事です..."
  }
}
```

### 8. `get_nostr_drafts`

自分の下書き記事を取得します。認証が必要です。

**パラメータ:**
| 名前 | 型 | 必須 | デフォルト | 説明 |
|------|------|------|------|------|
| `limit` | number | いいえ | 20 | 取得する下書きの最大数 (1-100) |

**例:**
```json
{
  "name": "get_nostr_drafts",
  "arguments": {
    "limit": 10
  }
}
```

## 使用例

Claude や Goose で使えるプロンプトの例：

- 「Nostr で今何が起きている？」 (get_nostr_timeline)
- 「『おはようございます、Nostr！』とノートを投稿して」 (post_nostr_note)
- 「Nostr でビットコインに関する議論を検索して」 (search_nostr_notes)
- 「npub1... は誰？プロフィールを取得して」 (get_nostr_profile)
- 「今日の Nostr のニュースを要約して投稿して」
- 「Bitcoin に関する最新の Nostr 記事を探して要約して」 (get_nostr_articles)
- 「Rust 勉強会の内容を長文記事として Nostr に投稿して」 (post_nostr_article)
- 「この記事を下書きとして保存して」 (save_nostr_draft)
- 「保存した下書きの一覧を見せて」 (get_nostr_drafts)

## セキュリティに関する推奨事項

### 推奨される使い分け

この MCP サーバーは、ニーズに応じて異なるセキュリティレベルをサポートしています：

#### 一般ユーザー向け（本実装）

- **適している用途:** 手軽さ重視、ローカル PC やサーバー環境での利用
- **設定方法:** 設定ファイルに秘密鍵を保存
- **ユースケース:**
  - 「今日の Nostr のニュースを要約して」
  - 「日報を投稿して」
  - 個人的な自動化や AI アシスタント
- **セキュリティ:** 秘密鍵はファイルシステムに保存されます。適切なファイル権限を設定してください（例: `chmod 600 config.json`）

#### 高セキュリティユーザー向け（別実装）

- **適している用途:** ファイルシステムに鍵を保存したくないユーザー
- **必要なもの:** KeePassXC 連携
- **ユースケース:**
  - 投稿ごとに人間の承認が必要な場合
  - AI に直接署名権限を渡したくない場合
  - 共有環境やマルチユーザー環境
- **設定方法:** 以下を行う別の MCP サーバーを実装:
  1. 各操作で KeePassXC に署名をリクエスト
  2. 署名リクエストごとにユーザー確認を要求
  3. 秘密鍵をディスクに保存しない

### ベストプラクティス

1. **設定ファイルをコミットしない** - `.gitignore` に追加
2. **専用の Nostr 鍵を使用** - メインのアイデンティティは使わない
3. **AI のアクションを確認** - AI があなたの代わりに何を投稿しているか監視
4. **リレーの露出を制限** - 信頼できるリレーにのみ接続
5. **定期的な鍵のローテーション** - 定期的に鍵を更新することを検討

## 開発

### ビルド

```bash
# デバッグビルド
cargo build

# リリースビルド
cargo build --release

# ログ付きで実行
RUST_LOG=debug cargo run
```

### サーバーのテスト

stdin 経由で JSON-RPC リクエストを送信してテストできます：

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | cargo run
```

MCP インスペクターを使用したテスト：

```bash
npx @anthropics/mcp-inspector cargo run
```

### プロジェクト構成

```
nostr-mcp-server/
├── Cargo.toml           # プロジェクト依存関係
├── README.md            # このファイル
├── CLAUDE.md            # 開発計画
├── LICENSE              # ライセンス
└── src/
    ├── main.rs          # エントリーポイントと設定
    ├── config.rs        # 設定ファイル管理
    ├── mcp.rs           # MCP サーバー実装
    ├── nostr_client.rs  # Nostr SDK ラッパー
    └── tools.rs         # ツール定義と実行
```

## 依存関係

- [nostr-sdk](https://github.com/rust-nostr/nostr) - Nostr プロトコル実装
- [tokio](https://tokio.rs/) - 非同期ランタイム
- [serde](https://serde.rs/) - シリアライゼーション/デシリアライゼーション
- [dotenvy](https://github.com/allan2/dotenvy) - 環境変数読み込み
- [anyhow](https://github.com/dtolnay/anyhow) - エラーハンドリング
- [tracing](https://github.com/tokio-rs/tracing) - ロギング
- [chrono](https://github.com/chronotope/chrono) - 日時処理

## プロトコル

このサーバーは Model Context Protocol (MCP) を JSON-RPC 2.0 over stdio で実装しています。MCP の詳細は [MCP 仕様](https://spec.modelcontextprotocol.io/) を参照してください。

## トラブルシューティング

### 「読み取り専用モードではこの操作はできません」

設定ファイル `~/.config/rust-nostr-mcp/config.json` に有効な Nostr 秘密鍵 (nsec) を設定してください。

### 「プロフィールが見つかりません」

ユーザーがプロフィールメタデータ (Kind 0 イベント) を公開していないか、接続されているリレーでプロフィールが利用できない可能性があります。

### 接続タイムアウト

リレーを追加するか、ネットワーク接続を確認してください。サーバーはリレーの応答を最大10秒待機します。

### 検索結果が返されない

検索リレーが NIP-50 をサポートしていることを確認してください。すべてのリレーが検索機能を実装しているわけではありません。

### 記事の取得に失敗する

NIP-23 (Kind 30023) をサポートするリレーに接続していることを確認してください。`relay.nostr.band` は長文コンテンツの取得に対応しています。

## ライセンス

MIT ライセンス - 詳細は [LICENSE](LICENSE) を参照してください。

## 貢献

貢献は歓迎します！お気軽にプルリクエストを送信してください。

## 関連プロジェクト

- [nostr-sdk](https://github.com/rust-nostr/nostr) - Rust 用 Nostr SDK
- [rust-nostr.org](https://rust-nostr.org/) - rust-nostr のドキュメント
- [Goose](https://github.com/block/goose) - Block 社の AI コーディングアシスタント
- [algia](https://github.com/mattn/algia) - Nostr CLI クライアント

## 謝辞

- [Nostr](https://nostr.com/) コミュニティ
- [Anthropic](https://anthropic.com/) - Claude と MCP 仕様
- [Block](https://block.xyz/) - Goose
- rust-nostr エコシステムの貢献者
