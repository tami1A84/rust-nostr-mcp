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

---

## Phase 5: NIP-04 → NIP-EE 移行（MLS ベース E2EE メッセージング）

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

## Phase 6: NIP-46 リモートサイニング（Nostr Connect）

### 背景

現在の実装では秘密鍵を `config.json` にローカル保存しているが、NIP-46 を使えば:
- 秘密鍵をサーバーに一切保存しない
- モバイルウォレット（Primal 等）でリモート署名
- QR コードスキャンで簡単にログイン
- 権限の粒度制御（特定 Kind のみ署名許可等）

### 参考仕様

| 仕様 | URL |
|------|-----|
| NIP-46 | https://github.com/nostr-protocol/nips/blob/master/46.md |
| nostr-connect クレート | https://crates.io/crates/nostr-connect |
| rust-nostr ドキュメント | https://rust-nostr.org |

### 接続フロー

```
┌─────────────────┐     ┌──────────────┐     ┌──────────────────┐
│   Goose チャット   │     │  MCP サーバー  │     │  Primal モバイル   │
│  (MCP クライアント) │     │ (rust-nostr-mcp)│    │  (リモートサイナー) │
└────────┬────────┘     └──────┬───────┘     └────────┬─────────┘
         │                     │                       │
         │  1. nostr_connect   │                       │
         │     ツール呼出       │                       │
         │────────────────────>│                       │
         │                     │                       │
         │  2. nostrconnect:// │                       │
         │     URI 生成         │                       │
         │                     │                       │
         │  3. QR コード表示    │                       │
         │     (MCP App UI)    │                       │
         │<────────────────────│                       │
         │                     │                       │
         │                     │  4. QR スキャン         │
         │                     │     → connect 要求     │
         │                     │<──────────────────────│
         │                     │                       │
         │                     │  5. Kind 24133 で      │
         │                     │     NIP-44 暗号化通信   │
         │                     │<─────────────────────>│
         │                     │                       │
         │  6. 接続完了通知     │                       │
         │<────────────────────│                       │
         │                     │                       │
         │  7. 以降のイベント   │  sign_event 要求      │
         │     署名は全て       │─────────────────────>│
         │     リモートサイナー  │  署名済みイベント返却  │
         │     経由             │<─────────────────────│
         └─────────────────────┴───────────────────────┘
```

### 接続方式

#### クライアント発行方式（メイン）
MCP サーバーが `nostrconnect://` URI を生成し、QR コードとして表示:
```
nostrconnect://<client-pubkey>?relay=wss://relay.damus.io&secret=<random>&perms=sign_event:1,sign_event:7,nip44_encrypt,nip44_decrypt&name=rust-nostr-mcp&url=https://github.com/tami1A84/rust-nostr-mcp
```

#### バンカー方式（オプション）
ユーザーが `bunker://` URI を config に設定:
```json
{
  "bunker-uri": "bunker://<signer-pubkey>?relay=wss://relay.damus.io&secret=<value>"
}
```

### NIP-46 メソッド対応

| メソッド | 用途 | 必要度 |
|----------|------|--------|
| `connect` | 接続確立 | 必須 |
| `get_public_key` | ユーザー公開鍵取得 | 必須 |
| `sign_event` | イベント署名 | 必須 |
| `ping` | 接続確認 | 必須 |
| `nip44_encrypt` | NIP-44 暗号化（NIP-EE 用） | 必須 |
| `nip44_decrypt` | NIP-44 復号（NIP-EE 用） | 必須 |
| `nip04_encrypt` | NIP-04 暗号化（後方互換） | オプション |
| `nip04_decrypt` | NIP-04 復号（後方互換） | オプション |

### 新しい依存クレート

```toml
# NIP-46 Nostr Connect
nostr-connect = "0.38"

# QR コード生成
qrcode = "0.14"
base64 = "0.22"
image = { version = "0.25", default-features = false, features = ["png"] }
```

### 実装ステップ

#### Step 6-1: NIP-46 クライアント基盤
- `nostr-connect` クレートを依存に追加
- `src/nip46.rs` を新規作成: Nostr Connect セッション管理
- `NostrConnect` signer の初期化・接続フロー
- 接続状態の管理（未接続 / 接続待ち / 接続済み）

#### Step 6-2: QR コード生成
- `qrcode` クレートで `nostrconnect://` URI を QR エンコード
- PNG 画像として生成 → Base64 エンコード
- MCP Apps の HTML テンプレートに埋め込み

#### Step 6-3: 認証モードの切り替え
- config に `auth-mode` フィールド追加:
  - `"local"`: 従来のローカル秘密鍵（デフォルト）
  - `"nip46"`: Nostr Connect リモートサイニング
  - `"bunker"`: bunker:// URI 指定
- NostrClient のサイナーを動的に切り替え

#### Step 6-4: MCP ツール追加
- `nostr_connect` ツール: QR コード生成・接続開始
- `nostr_connect_status` ツール: 接続状態確認
- `nostr_disconnect` ツール: リモートサイナー切断

### 設定形式の拡張

```json
{
  "relays": { ... },
  "privatekey": "nsec1...",
  "auth-mode": "nip46",
  "bunker-uri": "bunker://...",
  "nip46-relays": ["wss://relay.nsec.app"],
  "nip46-perms": "sign_event:1,sign_event:7,nip44_encrypt,nip44_decrypt",
  "nwc-uri": "nostr+walletconnect://..."
}
```

### 新ツール定義

| ツール名 | 説明 |
|----------|------|
| `nostr_connect` | NIP-46 接続を開始し QR コードを表示 |
| `nostr_connect_status` | リモートサイナーの接続状態を確認 |
| `nostr_disconnect` | リモートサイナーとの接続を切断 |

### コード構成の変更

```
src/
├── ...（既存ファイル）
└── nip46.rs         # NIP-46 Nostr Connect セッション管理 (新規)
```

---

## Phase 7: MCP Apps 対応（Goose チャット UI 埋め込み）

### 背景

MCP Apps (SEP-1865) は MCP の公式拡張仕様で、MCP サーバーがインタラクティブな UI をチャットクライアント内に埋め込むことを可能にする。Goose、ChatGPT、Claude Desktop、VS Code が対応済み。

`ui://` URI スキームで UI リソースを宣言し、ツール実行時にサンドボックス化された iframe として HTML コンテンツをレンダリングする。

### 参考仕様

| 仕様 | URL |
|------|-----|
| MCP Apps 仕様 (2026-01-26) | https://github.com/modelcontextprotocol/ext-apps/blob/main/specification/2026-01-26/apps.mdx |
| MCP Apps SDK | https://github.com/modelcontextprotocol/ext-apps |
| MCP Apps API | https://modelcontextprotocol.github.io/ext-apps/api/ |
| MCP Apps ブログ | https://blog.modelcontextprotocol.io/posts/2026-01-26-mcp-apps/ |

### アーキテクチャ

```
┌──────────────────────────────────────────────────────┐
│                  Goose チャット UI                      │
│                                                      │
│  ┌─────────────────────────────────────────────┐     │
│  │ 🔗 Nostr Connect (NIP-46)                   │     │
│  │  ┌───────────┐                              │     │
│  │  │  QR Code  │  Primal でスキャンしてログイン  │     │
│  │  │  ██████   │                              │     │
│  │  │  ██  ██   │  接続状態: 待機中...           │     │
│  │  │  ██████   │                              │     │
│  │  └───────────┘                              │     │
│  └─────────────────────────────────────────────┘     │
│                                                      │
│  ┌─────────────────────────────────────────────┐     │
│  │ 📄 記事プレビュー                              │     │
│  │  ┌─────────────────────────────────────┐    │     │
│  │  │ Title: Nostr と MLS の未来           │    │     │
│  │  │ Author: @fiatjaf · 2h ago           │    │     │
│  │  │                                     │    │     │
│  │  │ 記事本文のプレビュー...                │    │     │
│  │  │                                     │    │     │
│  │  │ #nostr #mls #encryption             │    │     │
│  │  │                                     │    │     │
│  │  │ [⚡ Zap 21 sats] [💬 Reply] [🔗 Open]│    │     │
│  │  └─────────────────────────────────────┘    │     │
│  └─────────────────────────────────────────────┘     │
│                                                      │
└──────────────────────────────────────────────────────┘
         │                    ▲
         │ tools/call         │ ui/notifications
         │ (iframe→host)      │ (host→iframe)
         ▼                    │
┌──────────────────────────────────────────────────────┐
│              rust-nostr-mcp (MCP サーバー)             │
│                                                      │
│  UI Resources:                                       │
│  ├── ui://nostr-mcp/connect-qr    (QR コード表示)     │
│  ├── ui://nostr-mcp/article-card  (記事プレビュー)    │
│  ├── ui://nostr-mcp/note-card     (ノートカード)      │
│  ├── ui://nostr-mcp/zap-button    (Zap ボタン)       │
│  └── ui://nostr-mcp/profile-card  (プロフィール)      │
│                                                      │
│  Tools with UI metadata:                             │
│  ├── nostr_connect    → ui://nostr-mcp/connect-qr   │
│  ├── get_nostr_articles → ui://nostr-mcp/article-card│
│  ├── get_nostr_timeline → ui://nostr-mcp/note-card   │
│  ├── send_zap         → ui://nostr-mcp/zap-button   │
│  └── get_nostr_profile → ui://nostr-mcp/profile-card│
└──────────────────────────────────────────────────────┘
```

### MCP Apps の仕組み

1. **UI リソース宣言**: サーバーが `ui://` スキームで HTML テンプレートを登録
2. **ツールと UI の紐付け**: ツール定義に `_meta.ui.resourceUri` を追加
3. **レンダリング**: ホスト（Goose）がツール実行結果を iframe 内の HTML に渡す
4. **双方向通信**: iframe 内の JS が `postMessage` + JSON-RPC でホスト経由でツール呼出可能

### UI リソース定義

```rust
// resources/list レスポンスに含める
{
  "uri": "ui://nostr-mcp/article-card",
  "name": "Nostr Article Preview",
  "description": "記事のリッチプレビューカード",
  "mimeType": "text/html;profile=mcp-app",
  "_meta": {
    "ui": {
      "csp": {
        "connectDomains": [],
        "resourceDomains": ["*"]  // 画像読み込み用
      }
    }
  }
}
```

### ツールの UI メタデータ

```rust
// tools/list レスポンスのツール定義に追加
{
  "name": "get_nostr_articles",
  "description": "...",
  "inputSchema": { ... },
  "_meta": {
    "ui": {
      "resourceUri": "ui://nostr-mcp/article-card",
      "visibility": ["model", "app"]
    }
  }
}
```

### 実装する UI コンポーネント

#### 7-1: NIP-46 QR コード接続画面 (`ui://nostr-mcp/connect-qr`)
- QR コードを中央に大きく表示
- `nostrconnect://` URI をテキストでも表示（コピー可能）
- 接続状態のリアルタイム表示（待機中 → 接続済み）
- 「Primal でスキャンしてログイン」の説明テキスト
- 接続完了時にユーザー情報を表示

#### 7-2: 記事プレビューカード (`ui://nostr-mcp/article-card`)
- タイトル・著者・公開日のヘッダー
- 記事本文の Markdown レンダリング（プレビュー）
- 画像・メディアの埋め込み表示
- ハッシュタグのバッジ表示
- Zap ボタン・リアクションボタン・共有ボタン
- naddr リンクでの外部クライアント連携

#### 7-3: ノートカード (`ui://nostr-mcp/note-card`)
- プロフィール画像・表示名・NIP-05
- ノート本文（画像・動画埋め込み対応）
- リアクション数・リプライ数・Zap 合計
- Zap ボタン・リアクションボタン・リプライボタン
- nevent リンク

#### 7-4: Zap ボタン (`ui://nostr-mcp/zap-button`)
- ⚡ アイコン付きの Zap ボタン
- 金額選択 UI（21, 100, 1000, カスタム sats）
- コメント入力欄（オプション）
- NWC 経由で `send_zap` ツールを呼び出し
- Zap 成功/失敗のフィードバック表示
- 既存の Zap レシート表示

#### 7-5: プロフィールカード (`ui://nostr-mcp/profile-card`)
- アバター・バナー画像
- 表示名・NIP-05 認証バッジ
- 自己紹介文
- フォロー数・フォロワー数・ノート数
- フォローボタン
- Lightning アドレス・Zap ボタン

### 双方向通信の実装例（Zap ボタン）

```html
<!-- ui://nostr-mcp/zap-button の HTML テンプレート -->
<script>
  // ホストからツール結果を受信
  window.addEventListener('message', (event) => {
    const msg = event.data;
    if (msg.method === 'ui/notifications/tool-result') {
      const result = msg.params.result;
      // Zap 結果を表示
      updateZapStatus(result);
    }
    if (msg.method === 'ui/notifications/tool-input') {
      // ノートデータを受け取って表示
      renderNote(msg.params.input);
    }
  });

  // Zap ボタンクリック時にツール呼出
  function sendZap(amount) {
    window.parent.postMessage({
      jsonrpc: "2.0",
      id: nextId++,
      method: "tools/call",
      params: {
        name: "send_zap",
        arguments: {
          target: currentNoteId,
          amount: amount,
          comment: document.getElementById('zap-comment').value
        }
      }
    }, '*');
  }
</script>
```

### 新しい依存クレート

```toml
# 追加の依存は不要（HTML テンプレートは文字列として Rust コード内に埋め込み）
# QR コード生成は Phase 6 で追加済み
```

### 実装ステップ

#### Step 7-1: MCP Apps 基盤
- `src/mcp_apps.rs` を新規作成: UI リソース管理
- MCP プロトコルハンドラ (`mcp.rs`) を拡張:
  - `resources/list` に UI リソースを返すよう実装
  - `resources/read` で `ui://` リソースの HTML を返す
  - ツール定義に `_meta.ui` を追加
- `io.modelcontextprotocol/ui` 拡張の宣言を `initialize` レスポンスに追加

#### Step 7-2: HTML テンプレートエンジン
- `ui/` ディレクトリに HTML テンプレートを配置
- ビルド時に `include_str!()` で Rust バイナリに埋め込み
- テンプレート内の `{{placeholder}}` をツール結果で置換
- テーマ対応（ホストの CSS 変数を利用）

#### Step 7-3: QR コード接続画面の実装
- Phase 6 の QR 生成と統合
- `nostr_connect` ツール実行時に QR を含む HTML を返す
- 接続状態のポーリング表示

#### Step 7-4: 記事・ノートカードの実装
- `get_nostr_articles` / `get_nostr_timeline` の結果を HTML カードとして表示
- Markdown → HTML レンダリング
- メディア埋め込み

#### Step 7-5: Zap ボタンの実装
- `send_zap` ツールと連携する Zap UI
- NWC 設定済みの場合のみ Zap ボタンを有効化
- 金額選択・コメント入力 → `tools/call` で `send_zap` を呼び出し

#### Step 7-6: プロフィールカードの実装
- `get_nostr_profile` の結果をリッチカードとして表示
- フォロー/Zap のインタラクション

### コード構成の変更

```
src/
├── ...（既存ファイル）
├── mcp_apps.rs      # MCP Apps UI リソース管理 (新規)
└── ui_templates.rs  # HTML テンプレート管理 (新規)

ui/
├── connect-qr.html    # NIP-46 QR コード接続画面
├── article-card.html  # 記事プレビューカード
├── note-card.html     # ノートカード
├── zap-button.html    # Zap ボタン UI
├── profile-card.html  # プロフィールカード
└── common.css         # 共通スタイル（テーマ対応）
```

---

## Phase 5-7 NIP サポートロードマップ（更新）

| NIP | 説明 | 状態 |
|-----|------|------|
| NIP-01 | 基本プロトコル | ✅ 実装済み |
| NIP-02 | コンタクトリスト | ✅ 実装済み |
| NIP-04 | 暗号化 DM（非推奨） | ⚠️ 読み取り専用で維持（Phase 5 で NIP-EE に移行） |
| NIP-05 | DNS 検証 | ✅ 実装済み |
| NIP-10 | リプライスレッディング | ✅ 実装済み |
| NIP-19 | bech32 エンコーディング | ✅ 実装済み |
| NIP-23 | 長文コンテンツ | ✅ 実装済み |
| NIP-25 | リアクション | ✅ 実装済み |
| NIP-27 | nostr: 参照 | ✅ 実装済み |
| NIP-44 | バージョン付き暗号ペイロード | 🔄 Phase 5 で追加（NIP-EE 基盤） |
| NIP-46 | Nostr Connect（リモートサイニング） | 🔄 Phase 6 で追加 |
| NIP-47 | Nostr Wallet Connect | ✅ 実装済み |
| NIP-50 | 検索 | ✅ 実装済み |
| NIP-57 | Zaps | ✅ 実装済み |
| NIP-65 | リレーリスト | ✅ 実装済み |
| NIP-EE | MLS E2EE メッセージング | 🔄 Phase 5 で追加 |
| MCP Apps | インタラクティブ UI 拡張 | 🔄 Phase 7 で追加 |

---

## Phase 5-7 実装優先度

| 優先度 | Phase | 機能 | 理由 |
|--------|-------|------|------|
| 🔴 高 | Phase 6 | NIP-46 リモートサイニング | セキュリティ向上の基盤。QR ログインで UX も改善 |
| 🔴 高 | Phase 7 | MCP Apps 基盤 + QR UI | NIP-46 の QR コード表示に必要 |
| 🟡 中 | Phase 7 | 記事/ノートカード | ユーザー体験の大幅改善 |
| 🟡 中 | Phase 7 | Zap ボタン UI | NWC と連携した直感的な Zap 体験 |
| 🟠 中 | Phase 5 | NIP-EE 移行 | セキュリティ改善。MLS エコシステムがまだ ALPHA のため |
| 🟢 低 | Phase 5 | グループメッセージング | NIP-EE 移行完了後に追加 |

### 推奨実装順序

1. **Phase 7 Step 7-1**: MCP Apps 基盤（UI リソースの仕組み構築）
2. **Phase 6 Step 6-1〜6-2**: NIP-46 + QR コード生成
3. **Phase 7 Step 7-3**: QR コード接続画面 UI
4. **Phase 6 Step 6-3〜6-4**: 認証モード切替 + ツール追加
5. **Phase 7 Step 7-4**: 記事・ノートカード UI
6. **Phase 7 Step 7-5**: Zap ボタン UI
7. **Phase 7 Step 7-6**: プロフィールカード UI
8. **Phase 5 Step 5-1〜5-2**: MLS 基盤 + NIP-EE イベント処理
9. **Phase 5 Step 5-3〜5-4**: DM 移行 + ストレージ

---

## 貢献

1. リポジトリをフォーク
2. フィーチャーブランチを作成
3. テスト付きで変更を実装
4. プルリクエストを送信

## ライセンス

MIT ライセンス
