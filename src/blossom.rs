//! Blossom メディアアップロードモジュール (NIP-B7)
//!
//! Blossom サーバーへのメディアファイルのアップロードを管理します。
//! BUD-02 に基づくアップロード API と、Kind 24242 認証イベントを使用します。

use anyhow::{anyhow, Context, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::debug;

/// デフォルトの Blossom サーバー
pub const DEFAULT_BLOSSOM_SERVERS: &[&str] = &[
    "https://blossom.primal.net",
    "https://nostr.download",
];

/// Blossom Blob Descriptor（BUD-02 レスポンス）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobDescriptor {
    /// 公開アクセス可能な Blob の URL
    pub url: String,
    /// Blob の SHA-256 ハッシュ（hex 形式）
    pub sha256: String,
    /// Blob のサイズ（バイト）
    pub size: u64,
    /// MIME タイプ
    #[serde(rename = "type", default)]
    pub content_type: String,
    /// アップロード日時（Unix タイムスタンプ）
    #[serde(default)]
    pub uploaded: u64,
}

/// ファイルデータの SHA-256 ハッシュを計算
pub fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// ファイル名の拡張子から MIME タイプを推測
pub fn guess_content_type(filename: &str) -> &'static str {
    match filename
        .rsplit('.')
        .next()
        .map(|s| s.to_lowercase())
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("avif") => "image/avif",
        Some("bmp") => "image/bmp",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mov") => "video/quicktime",
        Some("mp3") => "audio/mpeg",
        Some("ogg") => "audio/ogg",
        Some("wav") => "audio/wav",
        Some("flac") => "audio/flac",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    }
}

/// Blossom サーバーに Blob をアップロード（BUD-02）
///
/// # Arguments
/// * `server_url` - Blossom サーバーの URL（例: "https://blossom.primal.net"）
/// * `data` - アップロードするファイルデータ
/// * `content_type` - ファイルの MIME タイプ
/// * `auth_header` - `Authorization: Nostr <base64>` ヘッダーの値
pub async fn upload_blob(
    server_url: &str,
    data: Vec<u8>,
    content_type: &str,
    auth_header: &str,
) -> Result<BlobDescriptor> {
    let client = reqwest::Client::new();
    let url = format!("{}/upload", server_url.trim_end_matches('/'));

    debug!("Blossom アップロード: {} ({} bytes, {})", url, data.len(), content_type);

    let response = client
        .put(&url)
        .header("Content-Type", content_type)
        .header("Content-Length", data.len().to_string())
        .header("Authorization", auth_header)
        .body(data)
        .send()
        .await
        .context("Blossom サーバーへの接続に失敗")?;

    if !response.status().is_success() {
        let status = response.status();
        let reason = response
            .headers()
            .get("X-Reason")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("不明なエラー")
            .to_string();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!(
            "Blossom アップロードエラー ({}): {} - {}",
            status,
            reason,
            body
        ));
    }

    let descriptor: BlobDescriptor = response
        .json()
        .await
        .context("Blob Descriptor のパースに失敗")?;

    debug!("Blossom アップロード成功: {}", descriptor.url);
    Ok(descriptor)
}

/// 署名済み認証イベント JSON を Base64 エンコードして Authorization ヘッダー値を生成
pub fn create_auth_header(signed_event_json: &str) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(signed_event_json);
    format!("Nostr {}", encoded)
}
