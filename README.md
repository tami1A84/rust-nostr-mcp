# Nostr MCP Server

A Model Context Protocol (MCP) server that enables AI agents like Claude to interact with the Nostr network.

[日本語ドキュメント](#日本語) | [English Documentation](#overview)

---

## Overview

This server provides a bridge between AI assistants and the Nostr decentralized social network. It allows AI agents to:

- Post notes to Nostr
- Read timelines (personal or global)
- Search for notes
- Retrieve user profiles

## Installation

### Prerequisites

- Rust (latest stable version)
- A Nostr secret key (nsec) for write access (optional)

### Building from Source

```bash
git clone https://github.com/tami1A84/rust-nostr-mcp.git
cd rust-nostr-mcp
cargo build --release
```

The binary will be available at `target/release/nostr-mcp-server`.

## Configuration

### Environment Variables

Create a `.env` file in the project directory or set environment variables:

```bash
# Required for write access (posting notes)
# Use either NSEC or NOSTR_SECRET_KEY
NSEC=nsec1...

# Or in hex format
# NOSTR_SECRET_KEY=...

# Optional: Custom relay list (comma-separated)
NOSTR_RELAYS=wss://relay.damus.io,wss://nos.lol,wss://relay.nostr.band

# Optional: Search-capable relays for NIP-50 search (comma-separated)
NOSTR_SEARCH_RELAYS=wss://relay.nostr.band,wss://nostr.wine

# Optional: Logging level
RUST_LOG=info
```

### Default Relays

If not specified, the server uses these relays:

**General relays:**
- `wss://relay.damus.io`
- `wss://nos.lol`
- `wss://relay.nostr.band`
- `wss://nostr.wine`
- `wss://relay.snort.social`

**Search relays (NIP-50):**
- `wss://relay.nostr.band`
- `wss://nostr.wine`

## Usage

### Claude Desktop Configuration

Add the following to your Claude Desktop configuration file:

**macOS:** `~/Library/Application Support/Claude/claude_desktop_config.json`

**Windows:** `%APPDATA%\Claude\claude_desktop_config.json`

**Linux:** `~/.config/Claude/claude_desktop_config.json`

```json
{
  "mcpServers": {
    "nostr": {
      "command": "/path/to/nostr-mcp-server",
      "env": {
        "NSEC": "nsec1..."
      }
    }
  }
}
```

### Goose Configuration

[Goose](https://github.com/block/goose) is an AI coding assistant by Block that supports MCP servers.

Add the following to your Goose configuration file:

**Location:** `~/.config/goose/config.yaml`

```yaml
extensions:
  nostr:
    name: nostr
    type: stdio
    enabled: true
    cmd: /path/to/nostr-mcp-server
    env:
      NSEC: "nsec1..."
```

Or configure via CLI:

```bash
goose configure
# Select "Add Extension" -> "Command-line Extension"
# Name: nostr
# Command: /path/to/nostr-mcp-server
# Environment: NSEC=nsec1...
```

### Other MCP Clients

This server works with any MCP-compatible client. The server communicates via JSON-RPC 2.0 over stdio.

### Read-Only Mode

If no secret key is provided, the server runs in read-only mode. You can still:
- Get timelines (global)
- Search notes
- Get user profiles

But you cannot post notes.

## Available Tools

### 1. `post_note`

Post a new short text note (Kind 1) to the Nostr network.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `content` | string | Yes | The text content of the note to post |

**Example:**
```json
{
  "name": "post_note",
  "arguments": {
    "content": "Hello, Nostr!"
  }
}
```

### 2. `get_timeline`

Get the latest notes from the timeline. Returns notes from followed users if authenticated, otherwise returns the global timeline.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `limit` | integer | No | 20 | Maximum number of notes (1-100) |

**Example:**
```json
{
  "name": "get_timeline",
  "arguments": {
    "limit": 10
  }
}
```

### 3. `search_notes`

Search for notes containing specified keywords using NIP-50 search-capable relays.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `query` | string | Yes | - | The search query string |
| `limit` | integer | No | 20 | Maximum number of results (1-100) |

**Example:**
```json
{
  "name": "search_notes",
  "arguments": {
    "query": "bitcoin",
    "limit": 15
  }
}
```

### 4. `get_profile`

Get profile information for a Nostr user by their public key.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `npub` | string | Yes | Public key in npub (bech32) or hex format |

**Example:**
```json
{
  "name": "get_profile",
  "arguments": {
    "npub": "npub1..."
  }
}
```

## Use Cases

Here are some example prompts you can use with Claude:

- "What's happening on Nostr right now?" (get_timeline)
- "Post a note saying 'Good morning, Nostr!'" (post_note)
- "Search for discussions about Bitcoin on Nostr" (search_notes)
- "Who is npub1...? Get their profile." (get_profile)
- "Summarize today's news from Nostr and post a summary"
- "Post my daily report: [content]"

## Security Recommendations

### Recommended Usage Patterns

This MCP server supports different security levels depending on your needs:

#### General Users (This Implementation)

- **Best for:** Ease of use, local PC or server environments
- **Setup:** Store secret key in `.env` file
- **Use cases:**
  - "Summarize today's news from Nostr"
  - "Post my daily report"
  - Personal automation and AI assistance
- **Security:** The secret key is stored on the file system. Ensure proper file permissions (e.g., `chmod 600 .env`)

#### High-Security Users (Alternative Implementation)

- **Best for:** Users who don't want to store keys on the file system
- **Requires:** KeePassXC integration
- **Use cases:**
  - When you want human approval for every post
  - When AI should not have direct signing authority
  - Shared or multi-user environments
- **Setup:** Implement a separate MCP server that:
  1. Requests signatures from KeePassXC for each operation
  2. Requires user confirmation for every signing request
  3. Never stores the secret key on disk

### Best Practices

1. **Never commit your `.env` file** - Add it to `.gitignore`
2. **Use dedicated Nostr keys** - Don't use your main identity
3. **Review AI actions** - Monitor what the AI posts on your behalf
4. **Limit relay exposure** - Only connect to trusted relays
5. **Regular key rotation** - Consider rotating keys periodically

## Development

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run with logging
RUST_LOG=debug cargo run
```

### Testing the Server

You can test the server by sending JSON-RPC requests via stdin:

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | cargo run
```

### Project Structure

```
nostr-mcp-server/
├── Cargo.toml           # Project dependencies
├── README.md            # This file
├── LICENSE              # MIT License
└── src/
    ├── main.rs          # Entry point and configuration
    ├── mcp.rs           # MCP server implementation
    ├── nostr_client.rs  # Nostr SDK wrapper
    └── tools.rs         # Tool definitions and execution
```

## Dependencies

- [nostr-sdk](https://github.com/rust-nostr/nostr) - Nostr protocol implementation
- [tokio](https://tokio.rs/) - Async runtime
- [serde](https://serde.rs/) - Serialization/deserialization
- [dotenvy](https://github.com/allan2/dotenvy) - Environment variable loading
- [anyhow](https://github.com/dtolnay/anyhow) - Error handling
- [tracing](https://github.com/tokio-rs/tracing) - Logging

## Protocol

This server implements the Model Context Protocol (MCP) using JSON-RPC 2.0 over stdio. For more information about MCP, see the [MCP Specification](https://spec.modelcontextprotocol.io/).

## Troubleshooting

### "Cannot post notes in read-only mode"

Set the `NSEC` or `NOSTR_SECRET_KEY` environment variable with a valid Nostr secret key.

### "Profile not found"

The user may not have published their profile metadata (Kind 0 event), or the profile may not be available on the connected relays.

### Connection timeouts

Try adding more relays or checking your network connection. The server waits up to 10 seconds for relay responses.

### Search returns no results

Make sure the search relays support NIP-50. Not all relays implement search functionality.

## License

MIT License - See [LICENSE](LICENSE) for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Related Projects

- [nostr-sdk](https://github.com/rust-nostr/nostr) - The Nostr SDK for Rust
- [rust-nostr.org](https://rust-nostr.org/) - Documentation for rust-nostr
- [Goose](https://github.com/block/goose) - AI coding assistant by Block

## Acknowledgments

- The [Nostr](https://nostr.com/) community
- [Anthropic](https://anthropic.com/) for Claude and the MCP specification
- [Block](https://block.xyz/) for Goose
- Contributors to the rust-nostr ecosystem

---

# 日本語

## 概要

Nostr MCP Server は、Claude などの AI エージェントが Nostr ネットワークと対話できるようにする Model Context Protocol (MCP) サーバーです。

### 主な機能

- **ノートの投稿** - Nostr にショートテキストノート (Kind 1) を投稿
- **タイムラインの取得** - フォロー中のユーザーまたはグローバルタイムラインを取得
- **ノートの検索** - NIP-50 対応リレーでキーワード検索
- **プロフィールの取得** - ユーザーのプロフィール情報を取得

## インストール

### 必要条件

- Rust (最新の安定版)
- Nostr 秘密鍵 (nsec) - 書き込みアクセスに必要 (任意)

### ソースからビルド

```bash
git clone https://github.com/tami1A84/rust-nostr-mcp.git
cd rust-nostr-mcp
cargo build --release
```

バイナリは `target/release/nostr-mcp-server` に生成されます。

## 設定

### 環境変数

プロジェクトディレクトリに `.env` ファイルを作成するか、環境変数を設定します：

```bash
# 書き込みアクセスに必要 (ノート投稿用)
# NSEC または NOSTR_SECRET_KEY のいずれかを使用
NSEC=nsec1...

# または16進数形式
# NOSTR_SECRET_KEY=...

# オプション: カスタムリレーリスト (カンマ区切り)
NOSTR_RELAYS=wss://relay.damus.io,wss://nos.lol,wss://relay.nostr.band

# オプション: NIP-50 検索対応リレー (カンマ区切り)
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
      "command": "/path/to/nostr-mcp-server",
      "env": {
        "NSEC": "nsec1..."
      }
    }
  }
}
```

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
    env:
      NSEC: "nsec1..."
```

または CLI で設定：

```bash
goose configure
# "Add Extension" -> "Command-line Extension" を選択
# Name: nostr
# Command: /path/to/nostr-mcp-server
# Environment: NSEC=nsec1...
```

### 読み取り専用モード

秘密鍵が設定されていない場合、サーバーは読み取り専用モードで起動します。以下の機能は使用可能です：
- タイムラインの取得 (グローバル)
- ノートの検索
- プロフィールの取得

ただし、ノートの投稿はできません。

## 利用可能なツール

### 1. `post_note`

Nostr ネットワークにショートテキストノート (Kind 1) を投稿します。

**パラメータ:**
| 名前 | 型 | 必須 | 説明 |
|------|------|------|------|
| `content` | string | はい | 投稿するノートのテキスト内容 |

**例:**
```json
{
  "name": "post_note",
  "arguments": {
    "content": "こんにちは、Nostr！"
  }
}
```

### 2. `get_timeline`

タイムラインから最新のノートを取得します。認証済みの場合はフォロー中のユーザーのノート、それ以外はグローバルタイムラインを返します。

**パラメータ:**
| 名前 | 型 | 必須 | デフォルト | 説明 |
|------|------|------|------|------|
| `limit` | integer | いいえ | 20 | 取得するノートの最大数 (1-100) |

**例:**
```json
{
  "name": "get_timeline",
  "arguments": {
    "limit": 10
  }
}
```

### 3. `search_notes`

NIP-50 対応リレーを使用してキーワードでノートを検索します。

**パラメータ:**
| 名前 | 型 | 必須 | デフォルト | 説明 |
|------|------|------|------|------|
| `query` | string | はい | - | 検索クエリ文字列 |
| `limit` | integer | いいえ | 20 | 結果の最大数 (1-100) |

**例:**
```json
{
  "name": "search_notes",
  "arguments": {
    "query": "ビットコイン",
    "limit": 15
  }
}
```

### 4. `get_profile`

公開鍵で Nostr ユーザーのプロフィール情報を取得します。

**パラメータ:**
| 名前 | 型 | 必須 | 説明 |
|------|------|------|------|
| `npub` | string | はい | npub (bech32) または16進数形式の公開鍵 |

**例:**
```json
{
  "name": "get_profile",
  "arguments": {
    "npub": "npub1..."
  }
}
```

## 使用例

Claude で使えるプロンプトの例：

- 「Nostr で今何が起きている？」 (get_timeline)
- 「『おはようございます、Nostr！』とノートを投稿して」 (post_note)
- 「Nostr でビットコインに関する議論を検索して」 (search_notes)
- 「npub1... は誰？プロフィールを取得して」 (get_profile)
- 「今日の Nostr のニュースを要約して投稿して」
- 「日報を投稿して: [内容]」

## セキュリティに関する推奨事項

### 推奨される使い分け

この MCP サーバーは、ニーズに応じて異なるセキュリティレベルをサポートしています：

#### 一般ユーザー向け (本実装)

- **適している用途:** 手軽さ重視、ローカル PC やサーバー環境での利用
- **設定方法:** `.env` ファイルに秘密鍵を保存
- **ユースケース:**
  - 「今日の Nostr のニュースを要約して」
  - 「日報を投稿して」
  - 個人的な自動化や AI アシスタント
- **セキュリティ:** 秘密鍵はファイルシステムに保存されます。適切なファイル権限を設定してください（例: `chmod 600 .env`）

#### 高セキュリティユーザー向け (別実装)

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

1. **`.env` ファイルをコミットしない** - `.gitignore` に追加
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

### プロジェクト構成

```
nostr-mcp-server/
├── Cargo.toml           # プロジェクト依存関係
├── README.md            # このファイル
├── LICENSE              # MIT ライセンス
└── src/
    ├── main.rs          # エントリーポイントと設定
    ├── mcp.rs           # MCP サーバー実装
    ├── nostr_client.rs  # Nostr SDK ラッパー
    └── tools.rs         # ツール定義と実行
```

## トラブルシューティング

### 「読み取り専用モードではノートを投稿できません」

`NSEC` または `NOSTR_SECRET_KEY` 環境変数に有効な Nostr 秘密鍵を設定してください。

### 「プロフィールが見つかりません」

ユーザーがプロフィールメタデータ (Kind 0 イベント) を公開していないか、接続されているリレーでプロフィールが利用できない可能性があります。

### 接続タイムアウト

リレーを追加するか、ネットワーク接続を確認してください。サーバーはリレーの応答を最大10秒待機します。

### 検索結果が返されない

検索リレーが NIP-50 をサポートしていることを確認してください。すべてのリレーが検索機能を実装しているわけではありません。

## ライセンス

MIT ライセンス - 詳細は [LICENSE](LICENSE) を参照してください。

## 貢献

貢献は歓迎します！お気軽にプルリクエストを送信してください。

## 関連プロジェクト

- [nostr-sdk](https://github.com/rust-nostr/nostr) - Rust 用 Nostr SDK
- [rust-nostr.org](https://rust-nostr.org/) - rust-nostr のドキュメント
- [Goose](https://github.com/block/goose) - Block 社の AI コーディングアシスタント

## 謝辞

- [Nostr](https://nostr.com/) コミュニティ
- [Anthropic](https://anthropic.com/) - Claude と MCP 仕様
- [Block](https://block.xyz/) - Goose
- rust-nostr エコシステムの貢献者
