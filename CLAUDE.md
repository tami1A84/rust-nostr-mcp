# Nostr MCP Server - Development Plan

## Overview

This is a Model Context Protocol (MCP) server that enables AI agents to interact with the Nostr network. The server follows security best practices by storing private keys locally and never passing them to AI agents.

## Current Features (v0.2.0)

### Security
- **Secure Key Management**: Private keys stored in `~/.config/rust-nostr-mcp/config.json`
- **Algia-compatible Configuration**: Following the same config format as algia CLI
- **Read-only Mode**: Server operates safely without private key configured

### Tools
- `post_nostr_note` - Post short text notes (Kind 1)
- `get_nostr_timeline` - Get timeline with author information
- `search_nostr_notes` - Search notes using NIP-50
- `get_nostr_profile` - Get user profile information

### Modern Display Format
- Author information included (name, display_name, picture, nip05)
- Relative timestamps (e.g., "5m ago", "2h ago")
- nevent links for easy reference

---

## Future Plans

### Phase 1: NIP-23 Long-form Content Support

#### Goals
Support for long-form articles (Kind 30023/30024) as defined in [NIP-23](https://github.com/nostr-protocol/nips/blob/master/23.md).

#### New Tools to Implement

```
post_nostr_article
- Post a long-form article (Kind 30023)
- Parameters:
  - title (string, required): Article title
  - content (string, required): Markdown content
  - summary (string, optional): Brief description
  - image (string, optional): Header image URL
  - tags (array, optional): Topic hashtags
  - published_at (number, optional): Unix timestamp

get_nostr_articles
- Fetch long-form articles
- Parameters:
  - author (string, optional): Filter by author pubkey
  - tags (array, optional): Filter by hashtags
  - limit (number, optional): Max results

save_nostr_draft
- Save article as draft (Kind 30024)
- Same parameters as post_nostr_article

get_nostr_drafts
- Get user's draft articles
```

#### Technical Implementation
- Add Kind 30023 and 30024 support to nostr_client.rs
- Parse and validate Markdown content
- Handle addressable events with `d` tag
- Support `naddr` encoding for article references

---

### Phase 2: Enhanced Timeline Features

#### Goals
Improve the timeline experience with reactions, replies, and threading.

#### New Tools

```
get_nostr_thread
- Get a note with its replies in threaded format
- Parameters:
  - note_id (string, required): Event ID or nevent
  - depth (number, optional): Reply depth to fetch

react_to_note
- Add a reaction to a note (Kind 7)
- Parameters:
  - note_id (string, required): Target event ID
  - reaction (string, optional): Reaction emoji (default: "+")

reply_to_note
- Post a reply to an existing note
- Parameters:
  - note_id (string, required): Parent event ID
  - content (string, required): Reply content

get_nostr_notifications
- Get mentions and reactions to user's notes
- Parameters:
  - since (number, optional): Unix timestamp
  - limit (number, optional): Max results
```

#### Technical Implementation
- Fetch reaction counts (Kind 7) for timeline notes
- Implement reply threading with proper `e` and `p` tags
- Add NIP-10 marker support for threading

---

### Phase 3: Modern UI/UX Enhancements

#### Goals
Make the output more AI-friendly and visually structured.

#### Improvements

1. **Structured Note Display**
   ```json
   {
     "display_card": {
       "header": "👤 Username (@nip05)",
       "content": "Note content here...",
       "footer": "⚡ 42 reactions · 💬 5 replies · 2h ago"
     }
   }
   ```

2. **Rich Media Support**
   - Parse image URLs from content
   - Detect video/audio links
   - Support nostr:// references

3. **Content Formatting**
   - Parse hashtags and mentions
   - Highlight quoted notes (NIP-27)
   - Format code blocks in long-form content

4. **Profile Cards**
   ```json
   {
     "profile_card": {
       "avatar": "picture_url",
       "name": "Display Name",
       "nip05": "user@domain.com",
       "bio": "About text...",
       "stats": {
         "following": 150,
         "followers": 500,
         "notes": 1234
       }
     }
   }
   ```

---

### Phase 4: Advanced Features

#### NIP Support Roadmap

| NIP | Description | Priority |
|-----|-------------|----------|
| NIP-01 | Basic protocol | ✅ Done |
| NIP-02 | Contact List | ✅ Done |
| NIP-05 | DNS Verification | ✅ Done |
| NIP-10 | Reply Threading | 🔜 Phase 2 |
| NIP-19 | bech32 Encoding | ✅ Done |
| NIP-23 | Long-form Content | 🔜 Phase 1 |
| NIP-25 | Reactions | 🔜 Phase 2 |
| NIP-27 | nostr: References | 🔜 Phase 3 |
| NIP-50 | Search | ✅ Done |
| NIP-57 | Zaps | 📋 Phase 4 |
| NIP-65 | Relay List | 📋 Phase 4 |

#### Zap Support (NIP-57)
```
send_zap
- Send a Lightning zap to a note or profile
- Parameters:
  - target (string, required): Event ID or pubkey
  - amount (number, required): Amount in sats
  - comment (string, optional): Zap comment

get_zap_receipts
- Get zap receipts for a note
- Parameters:
  - note_id (string, required): Event ID
```

#### Direct Messages (NIP-04/NIP-17)
```
send_dm
- Send encrypted direct message
- Parameters:
  - recipient (string, required): Recipient pubkey
  - content (string, required): Message content

get_dms
- Get direct message conversations
- Parameters:
  - with (string, optional): Filter by conversation partner
  - limit (number, optional): Max messages
```

---

## Use Cases

以下は、rust-nostr-mcpをMCPクライアント（Claude Desktop、Goose、mcp-appなど）と組み合わせて活用するユースケースの提案です。

### 1. NIP-23 長文コンテンツのプレビューと要約（Phase 1連携）

MCPクライアント上でNostrの長文記事（Kind 30023）を取得し、AIがリアルタイムにプレビュー・要約を生成するワークフロー。

**シナリオ例:**
```
ユーザー: 「Bitcoinに関する最新のNostr記事を探して要約して」

AI Agent:
1. search_nostr_notes で "bitcoin" を検索
2. get_nostr_articles で長文記事を取得（Phase 1実装後）
3. Markdown記事をパースし、要約を生成
4. mcp-app上で記事のプレビューカード表示
```

**活用場面:**
- 技術ブログ記事のリサーチと要約
- 特定トピックの長文記事の比較分析
- 記事の下書き（Kind 30024）のレビュー・校正支援

---

### 2. AIアシスタントによるNostr投稿ワークフロー

AIがユーザーの意図を理解し、適切な形式でNostrに投稿する対話型ワークフロー。

**シナリオ例:**
```
ユーザー: 「今日のRust勉強会の内容をNostrに投稿したい」

AI Agent:
1. ユーザーとの対話でメモや要点を整理
2. 短文投稿（Kind 1）か長文記事（Kind 30023）かを判断
3. 下書きを生成してユーザーに確認
4. post_nostr_note または post_nostr_article で投稿
```

**活用場面:**
- イベントレポートの作成・投稿
- 技術メモの整形と投稿
- 多言語での同時投稿（日本語→英語翻訳して投稿）

---

### 3. Nostrタイムラインの定期サマリー

タイムラインを取得してAIが要約し、重要な話題をハイライトするダイジェスト生成。

**シナリオ例:**
```
ユーザー: 「今日のNostrタイムラインで話題になっていることを教えて」

AI Agent:
1. get_nostr_timeline で最新ノートを取得
2. トピック別に分類（技術、ニュース、コミュニティなど）
3. 主要な議論やトレンドを要約
4. 注目すべきノートのneventリンクを提示
```

**活用場面:**
- 朝のニュースダイジェスト生成
- 特定コミュニティの動向把握
- フォロー中のユーザーの活動サマリー

---

### 4. プロフィール分析とネットワーク調査

`get_nostr_profile` を活用し、Nostrユーザーの情報を収集・分析するユースケース。

**シナリオ例:**
```
ユーザー: 「このnpubのユーザーについて教えて」

AI Agent:
1. get_nostr_profile でプロフィール情報を取得
2. search_nostr_notes でそのユーザーの投稿を検索
3. 活動内容、興味分野、投稿頻度を分析
4. プロフィールカードとして構造化表示
```

**活用場面:**
- 新しくフォローする相手の事前調査
- コミュニティ内の影響力のあるユーザーの発見
- NIP-05認証の確認を含むプロフィール検証

---

### 5. Nostrを活用したリサーチツール

NIP-50検索とAIの分析能力を組み合わせた調査・リサーチ支援。

**シナリオ例:**
```
ユーザー: 「Nostr上でのLightning Network関連の議論をまとめて」

AI Agent:
1. search_nostr_notes で "lightning network" を検索
2. 関連する投稿を時系列で整理
3. 賛否の論点を分類・要約
4. 主要な議論参加者のプロフィールを取得
5. レポートとして構造化出力
```

**活用場面:**
- 技術トピックの動向調査
- プロジェクトに対するコミュニティの反応分析
- 競合分析やマーケットリサーチ

---

### 6. スレッド会話のコンテキスト理解（Phase 2連携）

スレッド形式の議論を取得し、AIが文脈を理解した上で返信案を提案するワークフロー。

**シナリオ例:**
```
ユーザー: 「このスレッドの議論を読んで、返信を考えて」

AI Agent:
1. get_nostr_thread でスレッド全体を取得（Phase 2実装後）
2. 議論の流れと各参加者の立場を分析
3. 文脈に合った返信案を複数提示
4. ユーザーが選択した返信を reply_to_note で投稿
```

**活用場面:**
- 技術的な議論への参加支援
- 適切なトーンでの返信作成
- 複数言語でのスレッド参加

---

### 7. コンテンツモデレーション支援

タイムラインやスレッドのコンテンツをAIが分析し、モデレーション判断を支援。

**シナリオ例:**
```
リレー運営者: 「最近の投稿からスパムや不適切なコンテンツを検出して」

AI Agent:
1. get_nostr_timeline で最新投稿を取得
2. コンテンツの分類と分析
3. スパムパターンや問題のある投稿を報告
4. モデレーションアクションの提案
```

**活用場面:**
- コミュニティリレーの運営支援
- スパムフィルタリングの補助
- コンテンツポリシー適用の一貫性確保

---

### 8. クロスプラットフォーム発信の起点としてのNostr

Nostrへの投稿をベースに、他プラットフォーム向けのコンテンツを生成するワークフロー。

**シナリオ例:**
```
ユーザー: 「このNostr記事をブログ記事とSNS投稿に変換して」

AI Agent:
1. get_nostr_articles で元記事を取得
2. ブログ向けにHTML/Markdown形式で再構成
3. 短文SNS向けに要点をまとめた投稿を生成
4. 各プラットフォーム向けフォーマットで出力
```

**活用場面:**
- Nostrファーストのコンテンツ戦略
- 記事の多チャネル展開
- 長文記事から短文投稿シリーズの自動生成

---

### MCP クライアント別の活用

| MCPクライアント | 主な活用シナリオ |
|----------------|----------------|
| **Claude Desktop** | 対話型のNostr投稿・リサーチ、記事の下書き支援 |
| **Goose** | 開発者向け自動化、Nostrボットのプロトタイピング |
| **mcp-app** | ビジュアルなタイムライン表示、記事プレビュー |
| **カスタムMCPクライアント** | 特定用途のNostr連携アプリケーション構築 |

---

## Configuration Reference

### Config File Location
`~/.config/rust-nostr-mcp/config.json`

### Config Format (algia-compatible)
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
  "nwc-uri": "nostr+walletconnect://..."
}
```

### Relay Configuration Options
- `read`: Fetch events from this relay
- `write`: Publish events to this relay
- `search`: Use for NIP-50 search queries

---

## Development Guidelines

### Code Structure
```
src/
├── main.rs          # Entry point, config loading
├── config.rs        # Configuration management
├── mcp.rs           # MCP protocol handler
├── nostr_client.rs  # Nostr SDK wrapper
└── tools.rs         # Tool definitions and executors
```

### Adding New Tools

1. Add tool definition in `tools.rs`:
   ```rust
   ToolDefinition {
       name: "new_tool_name".to_string(),
       description: "Description".to_string(),
       input_schema: json!({ ... }),
   }
   ```

2. Add handler in `ToolExecutor::execute()`:
   ```rust
   "new_tool_name" => self.new_tool(arguments).await,
   ```

3. Implement the tool method:
   ```rust
   async fn new_tool(&self, arguments: Value) -> Result<Value> {
       // Implementation
   }
   ```

4. Add corresponding method in `nostr_client.rs` if needed.

### Testing
```bash
# Build
cargo build

# Run with debug logging
RUST_LOG=debug cargo run

# Test with MCP inspector
npx @anthropics/mcp-inspector cargo run
```

---

## Contributing

1. Fork the repository
2. Create a feature branch
3. Implement changes with tests
4. Submit a pull request

## License

MIT License
