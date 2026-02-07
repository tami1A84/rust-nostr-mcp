# Nostr MCP サーバー

AI アシスタント（Goose、Claude Desktop、VS Code 等）から Nostr を読み書きできるようにする MCP サーバーです。

## できること

- **投稿** - ノートや長文記事を Nostr に投稿
- **閲覧** - タイムラインの取得、ノートの検索、プロフィールの表示
- **会話** - スレッドの閲覧、リプライ、リアクション（いいね）
- **通知** - 自分へのメンションやリアクションの確認
- **記事管理** - 長文記事の投稿、下書きの保存・取得
- **Zap** - Lightning Zap の送受信（NWC 設定が必要）
- **DM** - 暗号化ダイレクトメッセージの送受信
- **メディアアップロード** - Blossom サーバーへの画像・動画・音声ファイルのアップロード（NIP-B7）
- **リモートサイニング** - NIP-46 で秘密鍵をサーバーに置かずにモバイルウォレットで署名
- **リッチ UI** - MCP Apps 対応クライアントでノートカード・記事プレビュー・Zap ボタン等をインタラクティブ表示

秘密鍵はローカルに保存され、AI には渡されません。NIP-46 を使えば秘密鍵をサーバーに一切保存しない運用も可能です。

## セットアップ

### 1. ビルド

```bash
git clone https://github.com/tami1A84/rust-nostr-mcp.git
cd rust-nostr-mcp
cargo build --release
```

> Rust がインストールされていない場合は [rustup.rs](https://rustup.rs/) からインストールしてください。

### 2. MCP クライアントに登録

#### Goose

`~/.config/goose/config.yaml` に追加：

```yaml
extensions:
  nostr:
    name: nostr
    type: stdio
    enabled: true
    cmd: /path/to/rust-nostr-mcp/target/release/nostr-mcp-server
```

#### Claude Desktop

`claude_desktop_config.json` に追加：

```json
{
  "mcpServers": {
    "nostr": {
      "command": "/path/to/rust-nostr-mcp/target/release/nostr-mcp-server"
    }
  }
}
```

設定ファイルの場所：
- macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
- Windows: `%APPDATA%\Claude\claude_desktop_config.json`

#### VS Code

`.vscode/settings.json` または VS Code の設定から MCP サーバーを追加してください。

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

### 4. NIP-46 リモートサイニングの設定（オプション）

秘密鍵をサーバーに保存せず、モバイルウォレット（Primal、Amber 等）で署名する方式です。

#### 方式 A: QR コードで接続（推奨）

`auth-mode` を `nip46` に設定：

```json
{
  "relays": { ... },
  "auth-mode": "nip46",
  "nip46-relays": ["wss://relay.nsec.app", "wss://relay.damus.io"]
}
```

AI アシスタントに「Nostr に接続して」と話しかけると、QR コードが表示されます。モバイルウォレットでスキャンするだけで接続完了です。

#### 方式 B: Bunker URI で接続

ウォレットから取得した `bunker://` URI を設定：

```json
{
  "relays": { ... },
  "auth-mode": "bunker",
  "bunker-uri": "bunker://<signer-pubkey>?relay=wss://relay.damus.io&secret=<value>"
}
```

### 5. NWC の設定（Zap を送りたい場合）

Lightning Zap を送信するには、NWC (Nostr Wallet Connect) URI を設定してください：

```json
{
  "relays": { ... },
  "privatekey": "nsec1...",
  "nwc-uri": "nostr+walletconnect://..."
}
```

NWC URI は Lightning ウォレット（Alby、Mutiny Wallet 等）から取得できます。

### 6. Blossom サーバーの設定（メディアアップロードしたい場合）

画像や動画を Blossom サーバーにアップロードするには、`blossom-servers` を設定してください：

```json
{
  "relays": { ... },
  "privatekey": "nsec1...",
  "blossom-servers": ["https://blossom.primal.net"]
}
```

設定しない場合はデフォルトで `blossom.primal.net` が使用されます。`set_blossom_servers` ツールで Kind 10063 イベントとして Nostr 上に公開することもできます。

## MCP Apps（リッチ UI）について

MCP Apps (SEP-1865) は MCP の公式拡張仕様で、ツール実行結果をインタラクティブな UI としてチャット内に表示します。本サーバーは以下の 5 つの UI コンポーネントを提供します。

### 提供する UI コンポーネント

| コンポーネント | 説明 | 対応ツール |
|---|---|---|
| **ノートカード** | ノートをリッチ表示（メディア埋め込み、リアクション数等） | `get_nostr_timeline`, `search_nostr_notes`, `get_nostr_thread` |
| **記事プレビュー** | 長文記事の Markdown プレビュー（ヘッダー画像、ワードカウント等） | `get_nostr_articles`, `get_nostr_drafts` |
| **プロフィールカード** | アバター・バナー・NIP-05 認証・フォロー数等の構造化表示 | `get_nostr_profile` |
| **Zap ボタン** | 金額選択・コメント入力付きの Lightning Zap UI | `send_zap`, `get_zap_receipts` |
| **QR コード接続画面** | NIP-46 リモートサイニングの QR コード表示・接続状態管理 | `nostr_connect`, `nostr_connect_status` |

### 対応 MCP クライアント

MCP Apps は以下のクライアントで利用できます：

- **Goose** (v1.19.0+) - Block 社の AI エージェント
- **Claude Desktop** - Anthropic の公式デスクトップアプリ
- **VS Code** (Insiders) - Microsoft のコードエディタ
- **ChatGPT** - OpenAI のチャットクライアント

MCP Apps 非対応のクライアントでも、UI なしのテキスト形式で全ツールが通常通り動作します。UI はプログレッシブエンハンスメント（対応環境でのみリッチ表示）として提供されます。

### 動作の仕組み

1. MCP クライアントが `initialize` 時に `io.modelcontextprotocol/ui` 拡張サポートを宣言
2. サーバーがツール定義に `_meta.ui.resourceUri` を付与し、`ui://` リソースを登録
3. ツール実行時、クライアントが `ui://` リソースの HTML を取得
4. サンドボックス化された iframe 内で HTML をレンダリング
5. iframe 内の JavaScript が `postMessage` + JSON-RPC でホストと双方向通信

## 使い方

AI アシスタントに話しかけるだけで使えます：

| やりたいこと | 指示例 |
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
| Zap を送る | 「このノートに 100 sats Zap して」 |
| DM を送る | 「npub1... に『こんにちは』と DM して」 |
| NIP-46 で接続する | 「Nostr に接続して」（QR コードが表示される） |
| リレーリストを確認 | 「npub1... のリレーリストを教えて」 |
| 画像をアップロード | 「この画像を Blossom にアップロードして」 |
| Blossom サーバーを確認 | 「自分の Blossom サーバーリストを見せて」 |

## ツール一覧

### 基本ツール

| ツール名 | 説明 | 秘密鍵 |
|---|---|---|
| `get_nostr_timeline` | タイムラインを取得 | 不要 |
| `search_nostr_notes` | ノートを検索（NIP-50） | 不要 |
| `get_nostr_profile` | プロフィールを取得 | 不要 |
| `post_nostr_note` | ノートを投稿 | 必要 |

### 記事管理（NIP-23）

| ツール名 | 説明 | 秘密鍵 |
|---|---|---|
| `get_nostr_articles` | 長文記事を取得 | 不要 |
| `post_nostr_article` | 長文記事を投稿 | 必要 |
| `save_nostr_draft` | 下書きを保存 | 必要 |
| `get_nostr_drafts` | 下書きを取得 | 必要 |

### 会話・通知（NIP-10 / NIP-25）

| ツール名 | 説明 | 秘密鍵 |
|---|---|---|
| `get_nostr_thread` | スレッド（リプライツリー）を取得 | 不要 |
| `get_nostr_notifications` | 通知を取得 | 必要 |
| `reply_to_note` | ノートに返信 | 必要 |
| `react_to_note` | リアクション（いいね）を送信 | 必要 |

### Zap（NIP-57）

| ツール名 | 説明 | 必要設定 |
|---|---|---|
| `send_zap` | Lightning Zap を送信 | 秘密鍵 + NWC |
| `get_zap_receipts` | Zap レシートを取得 | 不要 |

### ダイレクトメッセージ（NIP-04）

| ツール名 | 説明 | 秘密鍵 |
|---|---|---|
| `send_dm` | 暗号化 DM を送信 | 必要 |
| `get_dms` | DM 会話を取得・復号 | 必要 |

### リレー管理（NIP-65）

| ツール名 | 説明 | 秘密鍵 |
|---|---|---|
| `get_relay_list` | リレーリストを取得 | 不要 |

### メディアアップロード（NIP-B7 Blossom）

| ツール名 | 説明 | 秘密鍵 |
|---|---|---|
| `upload_media` | Blossom サーバーにメディアファイルをアップロード（BUD-02） | 必要 |
| `get_blossom_servers` | ユーザーの Blossom サーバーリスト（Kind 10063）を取得 | 不要 |
| `set_blossom_servers` | Blossom サーバーリスト（Kind 10063）を公開 | 必要 |

### リモートサイニング（NIP-46）

| ツール名 | 説明 | 秘密鍵 |
|---|---|---|
| `nostr_connect` | NIP-46 接続を開始し QR コードを表示 | 不要 |
| `nostr_connect_status` | リモートサイナーの接続状態を確認 | 不要 |
| `nostr_disconnect` | リモートサイナーとの接続を切断 | 不要 |

## 設定リファレンス

### 設定ファイルの場所

`~/.config/rust-nostr-mcp/config.json`

### 設定形式（algia 互換）

```json
{
  "relays": {
    "wss://relay.damus.io": {
      "read": true,
      "write": true,
      "search": false
    },
    "wss://relay.nostr.band": {
      "read": true,
      "write": true,
      "search": true
    }
  },
  "privatekey": "nsec1...",
  "auth-mode": "local",
  "bunker-uri": "bunker://...",
  "nip46-relays": ["wss://relay.nsec.app"],
  "nwc-uri": "nostr+walletconnect://...",
  "blossom-servers": ["https://blossom.primal.net"]
}
```

### 設定項目

| 項目 | 説明 | デフォルト |
|---|---|---|
| `relays` | リレーの接続設定（read/write/search） | 5 つのデフォルトリレー |
| `privatekey` | nsec 形式の秘密鍵 | なし（読み取り専用） |
| `auth-mode` | 認証モード: `local` / `nip46` / `bunker` | `local` |
| `bunker-uri` | NIP-46 bunker:// URI | なし |
| `nip46-relays` | NIP-46 通信用リレー | `relay.nsec.app`, `relay.damus.io` |
| `nwc-uri` | Nostr Wallet Connect URI（Zap 用） | なし |
| `blossom-servers` | Blossom サーバー URL リスト（メディアアップロード用） | `blossom.primal.net` |

### 環境変数（設定ファイルの代替）

| 環境変数 | 説明 |
|---|---|
| `NSEC` / `NOSTR_SECRET_KEY` | 秘密鍵 |
| `NOSTR_RELAYS` | リレー URL（カンマ区切り） |
| `NOSTR_SEARCH_RELAYS` | 検索用リレー URL（カンマ区切り） |

## NIP サポート

| NIP | 説明 | 状態 |
|---|---|---|
| NIP-01 | 基本プロトコル | 実装済み |
| NIP-02 | コンタクトリスト | 実装済み |
| NIP-04 | 暗号化 DM | 実装済み |
| NIP-05 | DNS 検証 | 実装済み |
| NIP-10 | リプライスレッディング | 実装済み |
| NIP-19 | bech32 エンコーディング | 実装済み |
| NIP-23 | 長文コンテンツ | 実装済み |
| NIP-25 | リアクション | 実装済み |
| NIP-27 | nostr: 参照 | 実装済み |
| NIP-46 | Nostr Connect（リモートサイニング） | 実装済み |
| NIP-47 | Nostr Wallet Connect | 実装済み |
| NIP-50 | 検索 | 実装済み |
| NIP-57 | Zaps | 実装済み |
| NIP-65 | リレーリスト | 実装済み |
| NIP-B7 | Blossom メディアアップロード | 実装済み |

## トラブルシューティング

**「読み取り専用モードではこの操作はできません」**
→ `~/.config/rust-nostr-mcp/config.json` に秘密鍵 (nsec) を設定するか、NIP-46 でリモートサイナーに接続してください。

**接続がタイムアウトする**
→ ネットワーク接続を確認してください。リレーの応答を最大10秒待機します。

**検索結果が返されない**
→ すべてのリレーが検索に対応しているわけではありません。`relay.nostr.band` の設定で `"search": true` になっていることを確認してください。

**NIP-46 の QR コードが表示されない**
→ MCP Apps 対応のクライアント（Goose v1.19.0+、Claude Desktop 等）を使用してください。非対応クライアントでは `nostrconnect://` URI がテキストで表示されます。

**Zap が送れない**
→ `config.json` に `nwc-uri` が設定されていることを確認してください。NWC URI は Lightning ウォレット（Alby 等）から取得できます。

**メディアのアップロードに失敗する**
→ 秘密鍵または NIP-46 接続が必要です。Blossom サーバーがダウンしている場合は、`config.json` の `blossom-servers` に別のサーバーを設定するか、`upload_media` の `server` パラメータで直接指定してください。

## 開発

```bash
# ビルド
cargo build

# デバッグログ付きで実行
RUST_LOG=debug cargo run

# MCP インスペクターでテスト
npx @anthropics/mcp-inspector cargo run
```

### コード構成

```
src/
├── main.rs          # エントリーポイント、設定読み込み
├── config.rs        # 設定管理（認証モード切り替え含む）
├── content.rs       # コンテンツ解析（メディア・ハッシュタグ・NIP-27 参照）
├── mcp.rs           # MCP プロトコルハンドラ（MCP Apps 拡張対応）
├── mcp_apps.rs      # MCP Apps UI リソース管理
├── nip46.rs         # NIP-46 Nostr Connect セッション管理
├── blossom.rs       # Blossom メディアアップロード (NIP-B7, BUD-02)
├── nostr_client.rs  # Nostr SDK ラッパー
├── tools.rs         # ツール定義とエグゼキュータ（26 ツール）
└── ui_templates.rs  # HTML テンプレート管理

ui/
├── common.css         # 共通スタイル（テーマ対応）
├── note-card.html     # ノートカード UI
├── article-card.html  # 記事プレビューカード UI
├── profile-card.html  # プロフィールカード UI
├── zap-button.html    # Zap ボタン UI
└── connect-qr.html    # NIP-46 QR コード接続画面 UI
```

## ライセンス

MIT
