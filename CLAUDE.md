# Nostr MCP サーバー - 開発計画

## 概要

これは Model Context Protocol (MCP) サーバーで、AI エージェントが Nostr ネットワークと対話できるようにします。秘密鍵をローカルに保存し、AI エージェントには渡さないセキュリティベストプラクティスに従っています。

## 現在の機能 (v0.5.0)

### セキュリティ
- **安全な鍵管理**: 秘密鍵を `~/.config/rust-nostr-mcp/config.json` に保存
- **algia 互換設定**: algia CLI と同じ設定形式に準拠
- **読み取り専用モード**: 秘密鍵なしでも安全に動作

### ツール（基本）
- `post_nostr_note` - ショートテキストノート (Kind 1) を投稿
- `get_nostr_timeline` - 著者情報・リアクション数・リプライ数付きタイムラインを取得
- `search_nostr_notes` - NIP-50 を使用してノートを検索
- `get_nostr_profile` - ユーザープロフィール情報を取得

### ツール（Phase 1: NIP-23 長文コンテンツ）
- `post_nostr_article` - 長文記事 (Kind 30023) を投稿
- `get_nostr_articles` - 長文記事を取得（著者・タグでフィルタ可能）
- `save_nostr_draft` - 記事を下書き (Kind 30024) として保存
- `get_nostr_drafts` - ユーザーの下書き記事を取得

### ツール（Phase 2: タイムライン拡張）
- `get_nostr_thread` - スレッド形式でノートとリプライを階層取得（NIP-10）
- `react_to_note` - ノートにリアクション送信（NIP-25, Kind 7）
- `reply_to_note` - 既存ノートに返信（NIP-10 マーカー対応）
- `get_nostr_notifications` - メンション・リアクション通知を取得

### Phase 3: UI/UX の改善（実装済み）

#### 構造化ノート表示（display_card）
- ノートに `display_card` オブジェクトを追加（header, content, footer）
- header: 「表示名 (@nip05)」形式
- footer: 「N リアクション · N リプライ · 時間」形式

#### リッチメディアサポート
- コンテンツから画像 URL を自動検出（jpg, png, gif, webp, svg, avif 等）
- 動画 URL の検出（mp4, webm, mov 等）
- 音声 URL の検出（mp3, ogg, wav, flac 等）
- `media` オブジェクトとして出力（images, videos, audios）

#### コンテンツフォーマット（parsed_content）
- ハッシュタグの自動パース（#tag → hashtags 配列）
- Nostr 参照の検出（NIP-27: nostr:npub1..., nostr:note1..., nostr:nevent1... 等）
- 記事コンテンツにも同様の解析を適用

#### プロフィールカード（profile_card）
- `get_nostr_profile` に `profile_card` オブジェクトを追加
- avatar, name, nip05, bio を構造化表示
- 統計情報（stats）: following, followers, notes 数を取得・表示

### ツール（Phase 4: 高度な機能）
- `send_zap` - Lightning Zap を送信（NIP-57, NWC 設定が必要）
- `get_zap_receipts` - ノートの Zap レシートを取得（NIP-57）
- `send_dm` - 暗号化ダイレクトメッセージを送信（NIP-04）
- `get_dms` - DM 会話を取得・復号（NIP-04）
- `get_relay_list` - ユーザーのリレーリストを取得（NIP-65）

### モダンな表示形式
- 著者情報を含む（name、display_name、picture、nip05）
- 相対タイムスタンプ（例: 「5分前」「2時間前」）
- nevent リンクでの簡単な参照
- naddr エンコーディング対応（長文記事用）
- リアクション数・リプライ数のタイムライン表示

---

### Phase 4: 高度な機能（実装済み）

#### NIP サポートロードマップ

| NIP | 説明 | 状態 |
|-----|------|------|
| NIP-01 | 基本プロトコル | 実装済み |
| NIP-02 | コンタクトリスト | 実装済み |
| NIP-04 | 暗号化 DM | 実装済み |
| NIP-05 | DNS 検証 | 実装済み |
| NIP-10 | リプライスレッディング | 実装済み |
| NIP-19 | bech32 エンコーディング | 実装済み |
| NIP-23 | 長文コンテンツ | 実装済み |
| NIP-25 | リアクション | 実装済み |
| NIP-27 | nostr: 参照 | 実装済み |
| NIP-47 | Nostr Wallet Connect | 実装済み |
| NIP-50 | 検索 | 実装済み |
| NIP-57 | Zaps | 実装済み |
| NIP-65 | リレーリスト | 実装済み |

#### Zap サポート (NIP-57)
- `send_zap` - ノートまたはプロフィールに Lightning Zap を送信（NWC 設定が必要）
- `get_zap_receipts` - ノートの Zap レシートを取得（送信者・金額・コメント付き）

#### ダイレクトメッセージ (NIP-04)
- `send_dm` - 暗号化されたダイレクトメッセージを送信
- `get_dms` - ダイレクトメッセージの会話を取得・復号

#### リレーリスト (NIP-65)
- `get_relay_list` - ユーザーのリレーリスト (Kind 10002) を取得

---

## ユースケース

以下は、rust-nostr-mcp を MCP クライアント（Claude Desktop、Goose、mcp-app など）と組み合わせて活用するユースケースの提案です。

### 1. NIP-23 長文コンテンツのプレビューと要約

MCP クライアント上で Nostr の長文記事（Kind 30023）を取得し、AI がリアルタイムにプレビュー・要約を生成するワークフロー。

**シナリオ例:**
```
ユーザー: 「Bitcoin に関する最新の Nostr 記事を探して要約して」

AI Agent:
1. search_nostr_notes で "bitcoin" を検索
2. get_nostr_articles で長文記事を取得
3. Markdown 記事をパースし、要約を生成
4. mcp-app 上で記事のプレビューカード表示
```

**活用場面:**
- 技術ブログ記事のリサーチと要約
- 特定トピックの長文記事の比較分析
- 記事の下書き（Kind 30024）のレビュー・校正支援

---

### 2. AI アシスタントによる Nostr 投稿ワークフロー

AI がユーザーの意図を理解し、適切な形式で Nostr に投稿する対話型ワークフロー。

**シナリオ例:**
```
ユーザー: 「今日の Rust 勉強会の内容を Nostr に投稿したい」

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

### 3. Nostr タイムラインの定期サマリー

タイムラインを取得して AI が要約し、重要な話題をハイライトするダイジェスト生成。

**シナリオ例:**
```
ユーザー: 「今日の Nostr タイムラインで話題になっていることを教えて」

AI Agent:
1. get_nostr_timeline で最新ノートを取得
2. トピック別に分類（技術、ニュース、コミュニティなど）
3. 主要な議論やトレンドを要約
4. 注目すべきノートの nevent リンクを提示
```

**活用場面:**
- 朝のニュースダイジェスト生成
- 特定コミュニティの動向把握
- フォロー中のユーザーの活動サマリー

---

### 4. プロフィール分析とネットワーク調査

`get_nostr_profile` を活用し、Nostr ユーザーの情報を収集・分析するユースケース。

**シナリオ例:**
```
ユーザー: 「この npub のユーザーについて教えて」

AI Agent:
1. get_nostr_profile でプロフィール情報を取得
2. search_nostr_notes でそのユーザーの投稿を検索
3. 活動内容、興味分野、投稿頻度を分析
4. プロフィールカードとして構造化表示
```

**活用場面:**
- 新しくフォローする相手の事前調査
- コミュニティ内の影響力のあるユーザーの発見
- NIP-05 認証の確認を含むプロフィール検証

---

### 5. Nostr を活用したリサーチツール

NIP-50 検索と AI の分析能力を組み合わせた調査・リサーチ支援。

**シナリオ例:**
```
ユーザー: 「Nostr 上での Lightning Network 関連の議論をまとめて」

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

### 6. スレッド会話のコンテキスト理解（Phase 2 連携）

スレッド形式の議論を取得し、AI が文脈を理解した上で返信案を提案するワークフロー。

**シナリオ例:**
```
ユーザー: 「このスレッドの議論を読んで、返信を考えて」

AI Agent:
1. get_nostr_thread でスレッド全体を取得（Phase 2 実装後）
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

タイムラインやスレッドのコンテンツを AI が分析し、モデレーション判断を支援。

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

### 8. クロスプラットフォーム発信の起点としての Nostr

Nostr への投稿をベースに、他プラットフォーム向けのコンテンツを生成するワークフロー。

**シナリオ例:**
```
ユーザー: 「この Nostr 記事をブログ記事と SNS 投稿に変換して」

AI Agent:
1. get_nostr_articles で元記事を取得
2. ブログ向けに HTML/Markdown 形式で再構成
3. 短文 SNS 向けに要点をまとめた投稿を生成
4. 各プラットフォーム向けフォーマットで出力
```

**活用場面:**
- Nostr ファーストのコンテンツ戦略
- 記事の多チャネル展開
- 長文記事から短文投稿シリーズの自動生成

---

### MCP クライアント別の活用

| MCP クライアント | 主な活用シナリオ |
|----------------|----------------|
| **Claude Desktop** | 対話型の Nostr 投稿・リサーチ、記事の下書き支援 |
| **Goose** | 開発者向け自動化、Nostr ボットのプロトタイピング |
| **mcp-app** | ビジュアルなタイムライン表示、記事プレビュー |
| **カスタム MCP クライアント** | 特定用途の Nostr 連携アプリケーション構築 |

---

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
  "nwc-uri": "nostr+walletconnect://..."
}
```

### リレー設定オプション
- `read`: このリレーからイベントを取得
- `write`: このリレーにイベントを公開
- `search`: NIP-50 検索クエリに使用

### NWC (Nostr Wallet Connect) 設定
Zap 送信を有効にするには、`nwc-uri` フィールドに NWC URI を設定してください:
```json
{
  "nwc-uri": "nostr+walletconnect://..."
}
```
NWC URI は Lightning ウォレット（Alby、Mutiny Wallet 等）から取得できます。

---

## 開発ガイドライン

### コード構成
```
src/
├── main.rs          # エントリーポイント、設定読み込み
├── config.rs        # 設定管理
├── content.rs       # コンテンツ解析（メディア・ハッシュタグ・NIP-27 参照）
├── mcp.rs           # MCP プロトコルハンドラ
├── nostr_client.rs  # Nostr SDK ラッパー
└── tools.rs         # ツール定義とエグゼキュータ
```

### 新しいツールの追加方法

1. `tools.rs` にツール定義を追加:
   ```rust
   ToolDefinition {
       name: "new_tool_name".to_string(),
       description: "説明".to_string(),
       input_schema: json!({ ... }),
   }
   ```

2. `ToolExecutor::execute()` にハンドラを追加:
   ```rust
   "new_tool_name" => self.new_tool(arguments).await,
   ```

3. ツールメソッドを実装:
   ```rust
   async fn new_tool(&self, arguments: Value) -> Result<Value> {
       // 実装
   }
   ```

4. 必要に応じて `nostr_client.rs` に対応メソッドを追加。

### テスト
```bash
# ビルド
cargo build

# デバッグログ付きで実行
RUST_LOG=debug cargo run

# MCP インスペクターでテスト
npx @anthropics/mcp-inspector cargo run
```

---

## 貢献

1. リポジトリをフォーク
2. フィーチャーブランチを作成
3. テスト付きで変更を実装
4. プルリクエストを送信

## ライセンス

MIT ライセンス
