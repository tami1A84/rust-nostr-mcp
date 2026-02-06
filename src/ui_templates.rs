//! UI テンプレート管理モジュール
//!
//! MCP Apps 用の HTML テンプレートをビルド時に `include_str!()` で
//! バイナリに埋め込み、実行時にプレースホルダーを置換して提供します。

/// 共通 CSS スタイル（テーマ変数のフォールバック値付き）
const COMMON_CSS: &str = include_str!("../ui/common.css");

/// ノートカードテンプレート
const NOTE_CARD_HTML: &str = include_str!("../ui/note-card.html");

/// 記事プレビューカードテンプレート
const ARTICLE_CARD_HTML: &str = include_str!("../ui/article-card.html");

/// プロフィールカードテンプレート
const PROFILE_CARD_HTML: &str = include_str!("../ui/profile-card.html");

/// Zap ボタン UI テンプレート
const ZAP_BUTTON_HTML: &str = include_str!("../ui/zap-button.html");

/// NIP-46 QR コード接続画面テンプレート
const CONNECT_QR_HTML: &str = include_str!("../ui/connect-qr.html");

/// テンプレート名を列挙する定数
#[cfg(test)]
const TEMPLATE_NAMES: &[&str] = &[
    "note-card",
    "article-card",
    "profile-card",
    "zap-button",
    "connect-qr",
];

/// テンプレート名から生の HTML テンプレートを取得する
fn get_raw_template(name: &str) -> Option<&'static str> {
    match name {
        "note-card" => Some(NOTE_CARD_HTML),
        "article-card" => Some(ARTICLE_CARD_HTML),
        "profile-card" => Some(PROFILE_CARD_HTML),
        "zap-button" => Some(ZAP_BUTTON_HTML),
        "connect-qr" => Some(CONNECT_QR_HTML),
        _ => None,
    }
}

/// テンプレート名から処理済み HTML を取得する。
/// `{{COMMON_CSS}}` プレースホルダーを共通 CSS で置換する。
pub fn get_template(name: &str) -> Option<String> {
    get_raw_template(name).map(|html| html.replace("{{COMMON_CSS}}", COMMON_CSS))
}

/// テンプレートの説明を返す
pub fn get_template_description(name: &str) -> &'static str {
    match name {
        "note-card" => "Nostr ノートのリッチプレビューカード",
        "article-card" => "Nostr 長文記事のプレビューカード",
        "profile-card" => "Nostr ユーザープロフィールカード",
        "zap-button" => "Lightning Zap 送信 UI",
        "connect-qr" => "NIP-46 Nostr Connect QR コード接続画面",
        _ => "",
    }
}

/// テンプレートの表示名を返す
pub fn get_template_display_name(name: &str) -> &'static str {
    match name {
        "note-card" => "Nostr Note Card",
        "article-card" => "Nostr Article Preview",
        "profile-card" => "Nostr Profile Card",
        "zap-button" => "Nostr Zap Button",
        "connect-qr" => "Nostr Connect QR",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_templates_exist() {
        for name in TEMPLATE_NAMES {
            assert!(
                get_template(name).is_some(),
                "Template '{}' should exist",
                name
            );
        }
    }

    #[test]
    fn test_common_css_injected() {
        for name in TEMPLATE_NAMES {
            let html = get_template(name).unwrap();
            assert!(
                !html.contains("{{COMMON_CSS}}"),
                "Template '{}' should have CSS injected",
                name
            );
            // CSS should contain the body rule from common.css
            assert!(
                html.contains("box-sizing"),
                "Template '{}' should contain common CSS",
                name
            );
        }
    }

    #[test]
    fn test_unknown_template_returns_none() {
        assert!(get_template("nonexistent").is_none());
    }

    #[test]
    fn test_templates_are_valid_html() {
        for name in TEMPLATE_NAMES {
            let html = get_template(name).unwrap();
            assert!(
                html.contains("<!DOCTYPE html>"),
                "Template '{}' should be valid HTML5",
                name
            );
            assert!(
                html.contains("</html>"),
                "Template '{}' should have closing html tag",
                name
            );
        }
    }
}
