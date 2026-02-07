//! 設定モジュール
//!
//! ~/.config/rust-nostr-mcp/config.json からの設定の読み込みと保存を管理します。
//! algia の設定ファイル構造に準拠しています。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};

/// algia 規則に準拠したリレー設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayConfig {
    /// このリレーから読み取るかどうか
    pub read: bool,
    /// このリレーに書き込むかどうか
    pub write: bool,
    /// このリレーが NIP-50 検索をサポートするかどうか
    pub search: bool,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            read: true,
            write: true,
            search: false,
        }
    }
}

/// 認証モード
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthMode {
    /// 従来のローカル秘密鍵（デフォルト）
    Local,
    /// NIP-46 Nostr Connect リモートサイニング
    Nip46,
    /// bunker:// URI 指定
    Bunker,
}

impl Default for AuthMode {
    fn default() -> Self {
        Self::Local
    }
}

/// algia 規則に準拠したメイン設定構造体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// URL をキーとするリレー設定
    pub relays: HashMap<String, RelayConfig>,
    /// nsec 形式の秘密鍵（ローカルに保存、AI エージェントには渡されない）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub privatekey: Option<String>,
    /// Nostr Wallet Connect URI（任意）
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "nwc-uri")]
    pub nwc_uri: Option<String>,
    /// 認証モード: "local"（デフォルト）、"nip46"、"bunker"
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "auth-mode")]
    pub auth_mode: Option<AuthMode>,
    /// bunker:// URI（バンカー方式の場合）
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "bunker-uri")]
    pub bunker_uri: Option<String>,
    /// NIP-46 通信用リレー
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "nip46-relays")]
    pub nip46_relays: Option<Vec<String>>,
    /// NIP-46 要求権限（カンマ区切り: "sign_event:1,sign_event:7,nip44_encrypt"）
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "nip46-perms")]
    pub nip46_perms: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        let mut relays = HashMap::new();

        relays.insert(
            "wss://relay.damus.io".to_string(),
            RelayConfig { read: true, write: true, search: false },
        );
        relays.insert(
            "wss://nos.lol".to_string(),
            RelayConfig { read: true, write: true, search: false },
        );
        relays.insert(
            "wss://relay.nostr.band".to_string(),
            RelayConfig { read: true, write: true, search: true },
        );
        relays.insert(
            "wss://nostr.wine".to_string(),
            RelayConfig { read: true, write: false, search: true },
        );
        relays.insert(
            "wss://relay.snort.social".to_string(),
            RelayConfig { read: true, write: true, search: false },
        );

        Self {
            relays,
            privatekey: None,
            nwc_uri: None,
            auth_mode: None,
            bunker_uri: None,
            nip46_relays: None,
            nip46_perms: None,
        }
    }
}

impl Config {
    /// 設定ファイルのパスを取得
    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("設定ディレクトリを特定できません")?
            .join("rust-nostr-mcp");

        Ok(config_dir.join("config.json"))
    }

    /// 設定ファイルから設定を読み込みます。
    /// 後方互換性のため、環境変数へのフォールバックもサポートしています。
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            info!("設定ファイルを読み込み中: {:?}", config_path);
            let content = fs::read_to_string(&config_path)
                .context("設定ファイルの読み込みに失敗しました")?;
            let config: Config = serde_json::from_str(&content)
                .context("設定ファイルのパースに失敗しました")?;
            return Ok(config);
        }

        warn!("設定ファイルが見つかりません: {:?}。環境変数を確認します", config_path);
        Self::load_from_env()
    }

    /// 環境変数から設定を読み込みます（後方互換性）。
    fn load_from_env() -> Result<Self> {
        let _ = dotenvy::dotenv();

        let mut config = Self::default();

        if let Ok(nsec) = std::env::var("NSEC") {
            config.privatekey = Some(nsec);
        } else if let Ok(hex_key) = std::env::var("NOSTR_SECRET_KEY") {
            config.privatekey = Some(hex_key);
        }

        if let Ok(relay_list) = std::env::var("NOSTR_RELAYS") {
            config.relays.clear();
            for relay in relay_list.split(',').map(|s| s.trim()) {
                config.relays.insert(
                    relay.to_string(),
                    RelayConfig { read: true, write: true, search: false },
                );
            }
        }

        if let Ok(search_list) = std::env::var("NOSTR_SEARCH_RELAYS") {
            for relay in search_list.split(',').map(|s| s.trim()) {
                config.relays
                    .entry(relay.to_string())
                    .and_modify(|r| r.search = true)
                    .or_insert(RelayConfig { read: true, write: false, search: true });
            }
        }

        Ok(config)
    }

    /// 設定をファイルに保存します。
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .context("設定ディレクトリの作成に失敗しました")?;
        }

        let content = serde_json::to_string_pretty(self)
            .context("設定のシリアライズに失敗しました")?;

        fs::write(&config_path, content)
            .context("設定ファイルの書き込みに失敗しました")?;

        info!("設定を保存しました: {:?}", config_path);
        Ok(())
    }

    /// 設定ファイルが存在しない場合、デフォルト設定で作成します。
    pub fn create_default_if_missing() -> Result<bool> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            let default_config = Self::default();
            default_config.save()?;
            info!("デフォルト設定を作成しました: {:?}", config_path);
            return Ok(true);
        }

        Ok(false)
    }

    /// 指定条件に一致するリレー URL を取得する汎用ヘルパー
    fn relays_by<F: Fn(&RelayConfig) -> bool>(&self, predicate: F) -> Vec<String> {
        self.relays
            .iter()
            .filter(|(_, c)| predicate(c))
            .map(|(url, _)| url.clone())
            .collect()
    }

    /// 読み取り有効なリレー URL を取得
    pub fn read_relays(&self) -> Vec<String> {
        self.relays_by(|c| c.read)
    }

    /// 書き込み有効なリレー URL を取得
    #[allow(dead_code)]
    pub fn write_relays(&self) -> Vec<String> {
        self.relays_by(|c| c.write)
    }

    /// 検索有効なリレー URL を取得
    pub fn search_relays(&self) -> Vec<String> {
        self.relays_by(|c| c.search)
    }

    /// 有効な認証モードを取得（未指定の場合はデフォルト判定）
    pub fn effective_auth_mode(&self) -> AuthMode {
        if let Some(ref mode) = self.auth_mode {
            return mode.clone();
        }
        // auth-mode 未指定の場合、bunker-uri があれば Bunker、そうでなければ Local
        if self.bunker_uri.is_some() {
            AuthMode::Bunker
        } else {
            AuthMode::Local
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(!config.relays.is_empty());
        assert!(config.privatekey.is_none());
    }

    #[test]
    fn test_relay_filtering() {
        let config = Config::default();
        let read_relays = config.read_relays();
        let search_relays = config.search_relays();

        assert!(!read_relays.is_empty());
        assert!(!search_relays.is_empty());
    }
}
