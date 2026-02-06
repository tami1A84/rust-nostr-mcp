//! Nostr MCP サーバー
//!
//! AI エージェントが Nostr ネットワークと対話するための
//! Model Context Protocol (MCP) サーバーです。
//!
//! 設定は ~/.config/rust-nostr-mcp/config.json に保存されます。
//! 秘密鍵はローカルに保存され、AI エージェントには渡されません。

mod config;
mod content;
mod mcp;
mod mcp_apps;
mod nip46;
mod nostr_client;
mod tools;
mod ui_templates;

use anyhow::Result;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::config::{AuthMode, Config};
use crate::mcp::McpServer;
use crate::nip46::Nip46Config;
use crate::nostr_client::NostrClientConfig;

/// ログの初期化（tracing subscriber を使用）
/// MCP 通信（stdout）と干渉しないよう、ログは stderr に出力します。
fn init_logging() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_target(false)
                .compact(),
        )
        .init();
}

/// 設定ファイル (~/.config/rust-nostr-mcp/config.json) から設定を読み込みます。
/// 後方互換性のため、環境変数へのフォールバックもサポートしています。
fn load_config() -> NostrClientConfig {
    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            warn!("設定ファイルの読み込みに失敗しました。デフォルト設定を使用します: {}", e);
            Config::default()
        }
    };

    let secret_key = config.privatekey.clone();

    if secret_key.is_none() {
        warn!("秘密鍵が設定されていません。読み取り専用モードで起動します。");
        warn!("書き込みアクセスを有効にするには、nsec を設定ファイルに追加してください: {:?}", Config::config_path().unwrap_or_default());
    }

    let relays = config.read_relays();
    let search_relays = config.search_relays();
    let nwc_uri = config.nwc_uri.clone();
    let auth_mode = config.effective_auth_mode();

    if nwc_uri.is_some() {
        info!("  - NWC (Nostr Wallet Connect): 設定済み");
    }

    // NIP-46 設定の構築
    let nip46_config = match auth_mode {
        AuthMode::Nip46 | AuthMode::Bunker => {
            info!("  - 認証モード: {:?}", auth_mode);
            Some(Nip46Config {
                relays: config.nip46_relays.clone().unwrap_or_default(),
                perms: config.nip46_perms.clone(),
                bunker_uri: config.bunker_uri.clone(),
            })
        }
        AuthMode::Local => None,
    };

    NostrClientConfig {
        secret_key,
        relays,
        search_relays,
        nwc_uri,
        auth_mode,
        nip46_config,
    }
}

/// 初回起動時のセットアップ手順を表示します。
fn print_setup_instructions() {
    let config_path = Config::config_path().unwrap_or_default();
    eprintln!();
    eprintln!("=== Nostr MCP サーバー セットアップ ===");
    eprintln!();
    eprintln!("設定ファイル: {:?}", config_path);
    eprintln!();
    eprintln!("投稿を有効にするには、秘密鍵 (nsec) を設定ファイルに追加してください:");
    eprintln!();
    eprintln!("  {{");
    eprintln!("    \"relays\": {{");
    eprintln!("      \"wss://relay.damus.io\": {{ \"read\": true, \"write\": true, \"search\": false }}");
    eprintln!("    }},");
    eprintln!("    \"privatekey\": \"nsec1...\"");
    eprintln!("  }}");
    eprintln!();
    eprintln!("重要: 秘密鍵はローカルに保存され、AI エージェントには渡されません。");
    eprintln!();
}

#[tokio::main]
async fn main() -> Result<()> {
    init_logging();

    info!("Nostr MCP サーバーを起動中...");

    // 初回起動時にデフォルト設定ファイルを作成
    match Config::create_default_if_missing() {
        Ok(true) => print_setup_instructions(),
        Ok(false) => {}
        Err(e) => warn!("デフォルト設定の作成に失敗: {}", e),
    }

    let config = load_config();

    info!("設定を読み込みました:");
    info!("  - 読み取りリレー: {:?}", config.relays);
    info!("  - 検索リレー: {:?}", config.search_relays);
    info!("  - 書き込みアクセス: {}", if config.secret_key.is_some() { "有効" } else { "無効（読み取り専用）" });

    // MCP サーバーを作成して実行
    let server = McpServer::new(config).await?;
    server.run().await?;

    Ok(())
}
