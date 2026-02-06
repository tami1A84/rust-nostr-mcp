//! コンテンツ解析モジュール
//!
//! ノートや記事のコンテンツを解析し、メディア URL・ハッシュタグ・
//! Nostr 参照（NIP-27）を抽出します。

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// メディア情報（コンテンツから検出された画像・動画・音声 URL）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MediaInfo {
    /// 画像 URL のリスト
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<String>,
    /// 動画 URL のリスト
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub videos: Vec<String>,
    /// 音声 URL のリスト
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub audios: Vec<String>,
}

impl MediaInfo {
    /// メディアが一つも含まれていないか
    pub fn is_empty(&self) -> bool {
        self.images.is_empty() && self.videos.is_empty() && self.audios.is_empty()
    }
}

/// Nostr 参照情報（NIP-27: nostr: URI）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NostrReference {
    /// 参照の種類（npub, note, nevent, nprofile, naddr）
    #[serde(rename = "type")]
    pub ref_type: String,
    /// bech32 エンコードされた値
    pub bech32: String,
}

/// 解析済みコンテンツ
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParsedContent {
    /// 検出されたメディア
    #[serde(skip_serializing_if = "MediaInfo::is_empty")]
    pub media: MediaInfo,
    /// コンテンツ内のハッシュタグ
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hashtags: Vec<String>,
    /// Nostr 参照（NIP-27）
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<NostrReference>,
}

impl ParsedContent {
    /// 解析結果が空かどうか
    pub fn is_empty(&self) -> bool {
        self.media.is_empty() && self.hashtags.is_empty() && self.references.is_empty()
    }
}

// ========================================
// 正規表現パターン（遅延初期化）
// ========================================

/// URL 検出用の正規表現
fn url_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"https?://[^\s\)\]\}>，、。）」』】\x{3000}]+").unwrap()
    })
}

/// ハッシュタグ検出用の正規表現
fn hashtag_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // 行頭または空白の後の # に続く単語文字
        // キャプチャグループ 1 がハッシュタグ本体
        Regex::new(r"(?:^|\s)#([\w\p{L}\p{N}][\w\p{L}\p{N}_-]*)").unwrap()
    })
}

/// Nostr 参照（NIP-27）検出用の正規表現
fn nostr_ref_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"nostr:(npub1|note1|nevent1|nprofile1|naddr1)[a-z0-9]+").unwrap()
    })
}

// ========================================
// メディア分類用の拡張子リスト
// ========================================

/// 画像ファイルの拡張子
const IMAGE_EXTENSIONS: &[&str] = &[
    ".jpg", ".jpeg", ".png", ".gif", ".webp", ".svg", ".bmp", ".avif",
];

/// 動画ファイルの拡張子
const VIDEO_EXTENSIONS: &[&str] = &[
    ".mp4", ".webm", ".mov", ".avi", ".mkv",
];

/// 音声ファイルの拡張子
const AUDIO_EXTENSIONS: &[&str] = &[
    ".mp3", ".ogg", ".wav", ".flac", ".m4a", ".aac",
];

/// URL の拡張子からメディア種別を判定
fn classify_url(url: &str) -> Option<MediaType> {
    // クエリパラメータを除去して拡張子を判定
    let path = url.split('?').next().unwrap_or(url);
    let lower = path.to_lowercase();

    if IMAGE_EXTENSIONS.iter().any(|ext| lower.ends_with(ext)) {
        Some(MediaType::Image)
    } else if VIDEO_EXTENSIONS.iter().any(|ext| lower.ends_with(ext)) {
        Some(MediaType::Video)
    } else if AUDIO_EXTENSIONS.iter().any(|ext| lower.ends_with(ext)) {
        Some(MediaType::Audio)
    } else {
        None
    }
}

enum MediaType {
    Image,
    Video,
    Audio,
}

// ========================================
// 公開 API
// ========================================

/// コンテンツからメディア URL を抽出し分類する
pub fn extract_media(content: &str) -> MediaInfo {
    let mut media = MediaInfo::default();
    let re = url_regex();

    for m in re.find_iter(content) {
        let url = m.as_str().to_string();
        match classify_url(&url) {
            Some(MediaType::Image) => media.images.push(url),
            Some(MediaType::Video) => media.videos.push(url),
            Some(MediaType::Audio) => media.audios.push(url),
            None => {}
        }
    }

    media
}

/// コンテンツからハッシュタグを抽出する
pub fn extract_hashtags(content: &str) -> Vec<String> {
    let re = hashtag_regex();
    let mut tags: Vec<String> = re
        .captures_iter(content)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
        .collect();

    // 重複を除去
    tags.sort();
    tags.dedup();
    tags
}

/// コンテンツから Nostr 参照（NIP-27）を抽出する
pub fn extract_nostr_references(content: &str) -> Vec<NostrReference> {
    let re = nostr_ref_regex();

    re.find_iter(content)
        .map(|m| {
            let full = m.as_str();
            // "nostr:" プレフィックスを除去して bech32 値を取得
            let bech32 = &full[6..];
            let ref_type = if bech32.starts_with("npub1") {
                "npub"
            } else if bech32.starts_with("note1") {
                "note"
            } else if bech32.starts_with("nevent1") {
                "nevent"
            } else if bech32.starts_with("nprofile1") {
                "nprofile"
            } else if bech32.starts_with("naddr1") {
                "naddr"
            } else {
                "unknown"
            };

            NostrReference {
                ref_type: ref_type.to_string(),
                bech32: bech32.to_string(),
            }
        })
        .collect()
}

/// コンテンツを解析して構造化された情報を返す
pub fn parse_content(content: &str) -> ParsedContent {
    ParsedContent {
        media: extract_media(content),
        hashtags: extract_hashtags(content),
        references: extract_nostr_references(content),
    }
}

// ========================================
// テスト
// ========================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_media_images() {
        let content = "Check this out https://example.com/photo.jpg and https://example.com/pic.png";
        let media = extract_media(content);
        assert_eq!(media.images.len(), 2);
        assert!(media.images[0].contains("photo.jpg"));
        assert!(media.images[1].contains("pic.png"));
        assert!(media.videos.is_empty());
        assert!(media.audios.is_empty());
    }

    #[test]
    fn test_extract_media_videos() {
        let content = "Watch: https://example.com/video.mp4";
        let media = extract_media(content);
        assert_eq!(media.videos.len(), 1);
        assert!(media.images.is_empty());
    }

    #[test]
    fn test_extract_media_audios() {
        let content = "Listen: https://example.com/song.mp3 and https://example.com/track.ogg";
        let media = extract_media(content);
        assert_eq!(media.audios.len(), 2);
    }

    #[test]
    fn test_extract_media_with_query_params() {
        let content = "https://example.com/image.jpg?width=800&height=600";
        let media = extract_media(content);
        assert_eq!(media.images.len(), 1);
    }

    #[test]
    fn test_extract_media_mixed() {
        let content = "photo https://a.com/img.webp video https://b.com/clip.webm audio https://c.com/song.flac";
        let media = extract_media(content);
        assert_eq!(media.images.len(), 1);
        assert_eq!(media.videos.len(), 1);
        assert_eq!(media.audios.len(), 1);
    }

    #[test]
    fn test_extract_media_no_media() {
        let content = "Just a regular text note with no media";
        let media = extract_media(content);
        assert!(media.is_empty());
    }

    #[test]
    fn test_extract_media_url_without_media_extension() {
        let content = "Visit https://example.com/page and https://nostr.com";
        let media = extract_media(content);
        assert!(media.is_empty());
    }

    #[test]
    fn test_extract_hashtags() {
        let content = "Hello #nostr #bitcoin world";
        let tags = extract_hashtags(content);
        assert_eq!(tags, vec!["bitcoin", "nostr"]);
    }

    #[test]
    fn test_extract_hashtags_japanese() {
        let content = "#日本語 のハッシュタグ #テスト";
        let tags = extract_hashtags(content);
        assert!(tags.contains(&"テスト".to_string()));
        assert!(tags.contains(&"日本語".to_string()));
    }

    #[test]
    fn test_extract_hashtags_no_duplicates() {
        let content = "#nostr is great #nostr forever";
        let tags = extract_hashtags(content);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], "nostr");
    }

    #[test]
    fn test_extract_hashtags_no_match_in_url() {
        let content = "https://example.com/#section is not a hashtag";
        let tags = extract_hashtags(content);
        // URL 内の # はハッシュタグとして認識しない（前に空白がないため）
        assert!(tags.is_empty());
    }

    #[test]
    fn test_extract_nostr_references_npub() {
        let content = "Follow nostr:npub1abc123def456 for updates";
        let refs = extract_nostr_references(content);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].ref_type, "npub");
        assert_eq!(refs[0].bech32, "npub1abc123def456");
    }

    #[test]
    fn test_extract_nostr_references_note() {
        let content = "Check nostr:note1xyz789 out";
        let refs = extract_nostr_references(content);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].ref_type, "note");
    }

    #[test]
    fn test_extract_nostr_references_multiple() {
        let content = "From nostr:npub1abc to nostr:nevent1def about nostr:note1ghi";
        let refs = extract_nostr_references(content);
        assert_eq!(refs.len(), 3);
    }

    #[test]
    fn test_extract_nostr_references_naddr() {
        let content = "Article at nostr:naddr1abc123";
        let refs = extract_nostr_references(content);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].ref_type, "naddr");
    }

    #[test]
    fn test_parse_content_comprehensive() {
        let content = "Hello #nostr! Check nostr:npub1abc123 and https://example.com/photo.jpg";
        let parsed = parse_content(content);
        assert!(!parsed.hashtags.is_empty());
        assert!(!parsed.references.is_empty());
        assert!(!parsed.media.is_empty());
    }

    #[test]
    fn test_parse_content_empty() {
        let content = "Just plain text";
        let parsed = parse_content(content);
        assert!(parsed.is_empty());
    }
}
