# Nostr MCP サーバー

AI アシスタント (Goose) から Nostr を読み書きできるようにするツールです。

## できること

- **投稿** - ノートや長文記事を Nostr に投稿
- **閲覧** - タイムラインの取得、ノートの検索、プロフィールの表示
- **会話** - スレッドの閲覧、リプライ、リアクション（いいね）
- **通知** - 自分へのメンションやリアクションの確認
- **記事管理** - 長文記事の投稿、下書きの保存・取得

秘密鍵はローカルに保存され、AI には渡されません。

## セットアップ

### 1. ビルド

```bash
git clone https://github.com/tami1A84/rust-nostr-mcp.git
cd rust-nostr-mcp
cargo build --release
```

> Rust がインストールされていない場合は [rustup.rs](https://rustup.rs/) からインストールしてください。

### 2. Goose に登録

`~/.config/goose/config.yaml` に追加：

```yaml
extensions:
  nostr:
    name: nostr
    type: stdio
    enabled: true
    cmd: /path/to/rust-nostr-mcp/target/release/nostr-mcp-server
```

`/path/to/` は実際のパスに置き換えてください。

### 3. 秘密鍵の設定（投稿したい場合）

初回起動時に `~/.config/rust-nostr-mcp/config.json` が自動作成されます。
投稿機能を使うには、このファイルに秘密鍵を追加してください：

```json
{
  "relays": {
    "wss://relay.damus.io": { "read": true, "write": true, "search": false },
    "wss://nos.lol": { "read": true, "write": true, "search": false },
    "wss://relay.nostr.band": { "read": true, "write": true, "search": true }
  },
  "privatekey": "nsec1ここにあなたの秘密鍵を入れてください"
}
```

秘密鍵なしでも閲覧機能（タイムライン、検索、プロフィール）は使えます。

## 使い方

Goose に話しかけるだけで使えます：

| やりたいこと | Goose への指示例 |
|---|---|
| タイムラインを見る | 「Nostr のタイムラインを見せて」 |
| ノートを投稿する | 「『おはようございます』と Nostr に投稿して」 |
| ノートを検索する | 「Nostr でビットコインについて検索して」 |
| プロフィールを見る | 「npub1... のプロフィールを教えて」 |
| スレッドを見る | 「このノートのスレッドを見せて」 |
| リプライする | 「このノートに『ありがとう！』と返信して」 |
| いいねする | 「このノートにいいねして」 |
| 通知を確認する | 「Nostr の通知を確認して」 |
| 長文記事を書く | 「この内容を Nostr の長文記事として投稿して」 |
| 下書きを保存する | 「この記事を下書きとして保存して」 |

## ツール一覧

| ツール名 | 説明 | 秘密鍵 |
|---|---|---|
| `get_nostr_timeline` | タイムラインを取得 | 不要 |
| `search_nostr_notes` | ノートを検索 | 不要 |
| `get_nostr_profile` | プロフィールを取得 | 不要 |
| `get_nostr_articles` | 長文記事を取得 | 不要 |
| `get_nostr_thread` | スレッド（リプライツリー）を取得 | 不要 |
| `get_nostr_notifications` | 通知を取得 | 必要 |
| `post_nostr_note` | ノートを投稿 | 必要 |
| `reply_to_note` | ノートに返信 | 必要 |
| `react_to_note` | リアクション（いいね）を送信 | 必要 |
| `post_nostr_article` | 長文記事を投稿 | 必要 |
| `save_nostr_draft` | 下書きを保存 | 必要 |
| `get_nostr_drafts` | 下書きを取得 | 必要 |

## トラブルシューティング

**「読み取り専用モードではこの操作はできません」**
→ `~/.config/rust-nostr-mcp/config.json` に秘密鍵 (nsec) を設定してください。

**接続がタイムアウトする**
→ ネットワーク接続を確認してください。リレーの応答を最大10秒待機します。

**検索結果が返されない**
→ すべてのリレーが検索に対応しているわけではありません。`relay.nostr.band` の設定で `"search": true` になっていることを確認してください。

## ライセンス

MIT
