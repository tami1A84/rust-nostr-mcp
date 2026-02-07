# Nostr MCP サーバー - 開発計画

## 概要

これは Model Context Protocol (MCP) サーバーで、AI エージェントが Nostr ネットワークと対話できるようにします。秘密鍵をローカルに保存し、AI エージェントには渡さないセキュリティベストプラクティスに従っています。NIP-46 リモートサイニングにより、秘密鍵をサーバーに一切保存しない運用も可能です。

## 現在の機能 (v0.5.0)

### セキュリティ
- **安全な鍵管理**: 秘密鍵を `~/.config/rust-nostr-mcp/config.json` に保存
- **algia 互換設定**: algia CLI と同じ設定形式に準拠
- **読み取り専用モード**: 秘密鍵なしでも安全に動作
- **NIP-46 リモートサイニング**: 秘密鍵をサーバーに保存せず、モバイルウォレットで署名
- **3 つの認証モード**: ローカル秘密鍵 / NIP-46 QR 接続 / Bunker URI

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

### ツール（Phase 4: 高度な機能 - 実装済み）
- `send_zap` - Lightning Zap を送信（NIP-57, NWC 設定が必要）
- `get_zap_receipts` - ノートの Zap レシートを取得（NIP-57）
- `send_dm` - 暗号化ダイレクトメッセージを送信（NIP-04）
- `get_dms` - DM 会話を取得・復号（NIP-04）
- `get_relay_list` - ユーザーのリレーリストを取得（NIP-65）

### ツール（Phase 6: NIP-46 リモートサイニング - 実装済み）
- `nostr_connect` - NIP-46 接続を開始し QR コードを表示
- `nostr_connect_status` - リモートサイナーの接続状態を確認
- `nostr_disconnect` - リモートサイナーとの接続を切断

### Phase 7: MCP Apps 対応（実装済み）

MCP Apps (SEP-1865) に基づくインタラクティブ UI 拡張。MCP Apps 対応クライアント（Goose、Claude Desktop、VS Code、ChatGPT）でリッチ UI を表示。

#### UI コンポーネント（5 種）
- **ノートカード** (`ui://nostr-mcp/note-card`) - メディアグリッド、著者情報、タイムスタンプ付きノート表示
- **記事プレビュー** (`ui://nostr-mcp/article-card`) - Markdown レンダリング、ヘッダー画像、ワードカウント、下書きバッジ
- **プロフィールカード** (`ui://nostr-mcp/profile-card`) - アバター・バナー、NIP-05 認証、フォロー統計、Zap ボタン
- **Zap ボタン** (`ui://nostr-mcp/zap-button`) - 金額プリセット、カスタム入力、コメント、レシート表示
- **QR コード接続画面** (`ui://nostr-mcp/connect-qr`) - QR コード表示、URI コピー、接続状態ポーリング

#### ツールと UI のマッピング
| ツール | UI リソース |
|--------|------------|
| `get_nostr_timeline`, `search_nostr_notes`, `get_nostr_thread` | `note-card` |
| `get_nostr_articles`, `get_nostr_drafts` | `article-card` |
| `get_nostr_profile` | `profile-card` |
| `send_zap`, `get_zap_receipts` | `zap-button` |
| `nostr_connect`, `nostr_connect_status` | `connect-qr` |

### モダンな表示形式
- 著者情報を含む（name、display_name、picture、nip05）
- 相対タイムスタンプ（例: 「5分前」「2時間前」）
- nevent リンクでの簡単な参照
- naddr エンコーディング対応（長文記事用）
- リアクション数・リプライ数のタイムライン表示

---

## NIP サポートロードマップ

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
| NIP-46 | Nostr Connect（リモートサイニング） | 実装済み |
| NIP-47 | Nostr Wallet Connect | 実装済み |
| NIP-50 | 検索 | 実装済み |
| NIP-57 | Zaps | 実装済み |
| NIP-65 | リレーリスト | 実装済み |
| MCP Apps | インタラクティブ UI 拡張 (SEP-1865) | 実装済み |
| NIP-44 | バージョン付き暗号ペイロード | Phase 5 で追加予定 |
| NIP-EE | MLS E2EE メッセージング | Phase 5 で追加予定 |

---

## ユースケース

以下は、rust-nostr-mcp を MCP クライアント（Claude Desktop、Goose 等）と組み合わせて活用するユースケースです。

### 1. NIP-23 長文コンテンツのプレビューと要約

MCP クライアント上で Nostr の長文記事（Kind 30023）を取得し、AI がリアルタイムにプレビュー・要約を生成するワークフロー。MCP Apps 対応クライアントでは記事プレビューカードとしてリッチ表示されます。

**シナリオ例:**
```
ユーザー: 「Bitcoin に関する最新の Nostr 記事を探して要約して」

AI Agent:
1. search_nostr_notes で "bitcoin" を検索
2. get_nostr_articles で長文記事を取得
3. Markdown 記事をパースし、要約を生成
4. 記事プレビューカード（MCP Apps UI）でリッチ表示
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

`get_nostr_profile` を活用し、Nostr ユーザーの情報を収集・分析するユースケース。MCP Apps 対応クライアントではプロフィールカードとしてリッチ表示されます。

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

### 6. スレッド会話のコンテキスト理解

スレッド形式の議論を取得し、AI が文脈を理解した上で返信案を提案するワークフロー。

**シナリオ例:**
```
ユーザー: 「このスレッドの議論を読んで、返信を考えて」

AI Agent:
1. get_nostr_thread でスレッド全体を取得
2. 議論の流れと各参加者の立場を分析
3. 文脈に合った返信案を複数提示
4. ユーザーが選択した返信を reply_to_note で投稿
```

**活用場面:**
- 技術的な議論への参加支援
- 適切なトーンでの返信作成
- 複数言語でのスレッド参加

---

### 7. Lightning Zap による支援

Zap ボタン UI を使って、気に入ったノートや記事に Lightning Zap を送信するワークフロー。

**シナリオ例:**
```
ユーザー: 「このノートに 100 sats Zap して」

AI Agent:
1. send_zap でノートに Zap を送信
2. Zap ボタン UI（MCP Apps）で金額・コメントの確認
3. NWC 経由で Lightning 決済を実行
4. Zap レシートを表示
```

**活用場面:**
- コンテンツクリエイターへの支援
- 有用な情報への感謝の表現
- コミュニティ活動の促進

---

### 8. NIP-46 リモートサイニングによるセキュアな接続

QR コードをスキャンするだけで、秘密鍵をサーバーに保存せずに Nostr を利用するワークフロー。

**シナリオ例:**
```
ユーザー: 「Nostr に接続して」

AI Agent:
1. nostr_connect で QR コード接続画面を表示
2. ユーザーがモバイルウォレット（Primal 等）で QR をスキャン
3. NIP-46 で接続確立、以降のイベント署名はリモートサイナー経由
4. nostr_connect_status で接続状態を確認
```

**活用場面:**
- セキュアな初回セットアップ
- 共有デバイスでの一時的な利用
- 秘密鍵を一箇所に集約したい場合

---

### MCP クライアント別の活用

| MCP クライアント | 主な活用シナリオ | MCP Apps |
|----------------|----------------|----------|
| **Claude Desktop** | 対話型の Nostr 投稿・リサーチ、記事の下書き支援 | 対応 |
| **Goose** (v1.19.0+) | 開発者向け自動化、Nostr ボットのプロトタイピング | 対応 |
| **VS Code** (Insiders) | 開発中のドキュメント投稿、コミュニティ連携 | 対応 |
| **ChatGPT** | 対話型リサーチ、記事作成支援 | 対応 |
| **カスタム MCP クライアント** | 特定用途の Nostr 連携アプリケーション構築 | 実装依存 |

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
  "auth-mode": "local",
  "bunker-uri": "bunker://...",
  "nip46-relays": ["wss://relay.nsec.app"],
  "nwc-uri": "nostr+walletconnect://..."
}
```

### 設定項目

| 項目 | 説明 | デフォルト |
|------|------|-----------|
| `relays` | リレーの接続設定（read/write/search） | 5 つのデフォルトリレー |
| `privatekey` | nsec 形式の秘密鍵 | なし（読み取り専用） |
| `auth-mode` | 認証モード: `local` / `nip46` / `bunker` | `local` |
| `bunker-uri` | NIP-46 bunker:// URI | なし |
| `nip46-relays` | NIP-46 通信用リレー | `relay.nsec.app`, `relay.damus.io` |
| `nwc-uri` | Nostr Wallet Connect URI（Zap 用） | なし |

### リレー設定オプション
- `read`: このリレーからイベントを取得
- `write`: このリレーにイベントを公開
- `search`: NIP-50 検索クエリに使用

### 環境変数（設定ファイルの代替）
- `NSEC` / `NOSTR_SECRET_KEY`: 秘密鍵
- `NOSTR_RELAYS`: リレー URL（カンマ区切り）
- `NOSTR_SEARCH_RELAYS`: 検索用リレー URL（カンマ区切り）

---

## 開発ガイドライン

### コード構成
```
src/
├── main.rs          # エントリーポイント、設定読み込み
├── config.rs        # 設定管理（認証モード切り替え含む）
├── content.rs       # コンテンツ解析（メディア・ハッシュタグ・NIP-27 参照）
├── mcp.rs           # MCP プロトコルハンドラ（MCP Apps 拡張対応）
├── mcp_apps.rs      # MCP Apps UI リソース管理
├── nip46.rs         # NIP-46 Nostr Connect セッション管理
├── nostr_client.rs  # Nostr SDK ラッパー
├── tools.rs         # ツール定義とエグゼキュータ（23 ツール）
└── ui_templates.rs  # HTML テンプレート管理

ui/
├── common.css         # 共通スタイル（テーマ対応）
├── note-card.html     # ノートカード UI
├── article-card.html  # 記事プレビューカード UI
├── profile-card.html  # プロフィールカード UI
├── zap-button.html    # Zap ボタン UI
└── connect-qr.html    # NIP-46 QR コード接続画面 UI
```

### 新しいツールの追加方法

1. `tools.rs` にツール定義を追加:
   ```rust
   ToolDefinition {
       name: "new_tool_name".to_string(),
       description: "説明".to_string(),
       input_schema: json!({ ... }),
       meta: None, // MCP Apps UI が必要な場合は mcp_apps::get_tool_ui_meta() を使用
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

5. MCP Apps UI を追加する場合:
   - `ui/` ディレクトリに HTML テンプレートを作成
   - `ui_templates.rs` にテンプレート読み込みを追加
   - `mcp_apps.rs` の `UI_RESOURCES` と `TOOL_UI_MAPPINGS` に定義を追加

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

## Phase 5: NIP-04 → NIP-EE 移行（MLS ベース E2EE メッセージング）- 未実装

### 背景

NIP-04 は廃止（deprecated）されており、以下の問題がある:
- メタデータ（送信者・受信者）が平文で漏洩する
- 前方秘匿性（forward secrecy）がない
- 鍵の漏洩時に過去のメッセージが全て復号される
- グループメッセージングに対応していない

NIP-EE は MLS（Messaging Layer Security, RFC 9420）をベースとし、Nostr のリレーネットワーク上で安全なグループメッセージングを実現する。Marmot Protocol は NIP-EE を拡張し、メディアやグループ管理を標準化する。

### 参考仕様

| 仕様 | URL |
|------|-----|
| NIP-EE | https://github.com/nostr-protocol/nips/blob/master/EE.md |
| Marmot Protocol | https://github.com/marmot-protocol/marmot |
| RFC 9420 (MLS) | https://www.rfc-editor.org/rfc/rfc9420.html |
| MDK (Marmot Development Kit) | https://github.com/parres-hq/mdk |
| OpenMLS | https://github.com/openmls/openmls |
| openmls_nostr_crypto | https://github.com/erskingardner/openmls_nostr_crypto |

### NIP-EE イベント種別

| Kind | 用途 |
|------|------|
| 443 | KeyPackage Event（グループ招待の受信準備） |
| 444 | Welcome Event（グループへの招待） |
| 445 | Group Event（メッセージ・提案・コミット） |
| 10051 | KeyPackage Relays List（KeyPackage の配布リレー） |

### 暗号化方式

- NIP-44 暗号化（NIP-04 の AES-CBC を置き換え）
- MLS `exporter_secret` から Nostr 鍵ペアを導出し NIP-44 で暗号化
- エポックごとに鍵が自動ローテーション
- 各メッセージはエフェメラル鍵ペアで署名（送信者匿名化）

### セキュリティ特性

- **前方秘匿性**: 鍵漏洩しても過去のメッセージは安全
- **侵害後セキュリティ**: 鍵ローテーションにより将来の露出を制限
- **ID 分離**: MLS 署名鍵と Nostr ID 鍵は別
- **メタデータ保護**: エフェメラル鍵で送信者とグループサイズを隠蔽

### 新しい依存クレート

```toml
# MLS (Messaging Layer Security) - NIP-EE
openmls = "1"
openmls_nostr_crypto = "0.1"  # secp256k1 ベース MLS ciphersuite
nrc-mls = "0.1"               # Nostr + MLS 統合ライブラリ (ALPHA)
# または rust-nostr の nostr_mls が正式リリースされた場合はそちらを使用
```

### 実装ステップ

#### Step 5-1: MLS 基盤の追加
- `openmls` + `openmls_nostr_crypto` を依存に追加
- `src/mls.rs` を新規作成: MLS グループ管理・鍵管理のラッパー
- MLS KeyPackage の生成・署名
- グループ作成・参加・離脱のロジック

#### Step 5-2: NIP-EE イベント処理
- Kind 443 (KeyPackage) の生成・公開
- Kind 444 (Welcome) の送受信
- Kind 445 (Group Event) の暗号化・復号
- Kind 10051 (KeyPackage Relays) の管理
- NIP-44 暗号化への移行（NIP-04 AES-CBC を置き換え）

#### Step 5-3: 既存 DM ツールの移行
- `send_dm` を NIP-EE ベースに書き換え（1:1 は 2人グループとして実装）
- `get_dms` を NIP-EE メッセージ復号に対応
- 後方互換: NIP-04 メッセージの読み取りは維持（書き込みは NIP-EE のみ）
- 新ツール追加:
  - `create_group` - MLS グループの作成
  - `invite_to_group` - グループへのメンバー招待
  - `get_group_messages` - グループメッセージの取得・復号
  - `list_groups` - 参加中のグループ一覧

#### Step 5-4: ローカルストレージ
- MLS グループ状態の永続化（SQLite または JSON ファイル）
- KeyPackage・署名鍵のローカル管理
- エポック情報のキャッシュ

### 新ツール定義

| ツール名 | 説明 |
|----------|------|
| `send_dm` | E2EE ダイレクトメッセージ送信（NIP-EE ベース） |
| `get_dms` | E2EE メッセージ取得・復号（NIP-04 後方互換あり） |
| `create_group` | MLS 暗号化グループを作成 |
| `invite_to_group` | グループにメンバーを招待 |
| `get_group_messages` | グループメッセージを取得・復号 |
| `list_groups` | 参加中のグループ一覧を表示 |

### コード構成の変更

```
src/
├── ...（既存ファイル）
├── mls.rs           # MLS グループ管理・暗号化 (新規)
└── mls_storage.rs   # MLS 状態の永続化 (新規)
```

---

## 貢献

1. リポジトリをフォーク
2. フィーチャーブランチを作成
3. テスト付きで変更を実装
4. プルリクエストを送信

## ライセンス

MIT ライセンス
