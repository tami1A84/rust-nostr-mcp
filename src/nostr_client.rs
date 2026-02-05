//! Nostr Client Module
//!
//! Provides a wrapper around the nostr-sdk client with convenient methods
//! for the MCP tools.

use anyhow::{anyhow, Context, Result};
use nostr_sdk::prelude::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Configuration for the Nostr client.
#[derive(Debug, Clone)]
pub struct NostrClientConfig {
    /// Secret key in nsec or hex format (optional for read-only mode)
    pub secret_key: Option<String>,
    /// List of relay URLs for general operations
    pub relays: Vec<String>,
    /// List of relay URLs that support search (NIP-50)
    pub search_relays: Vec<String>,
}

/// Wrapper around the nostr-sdk client.
pub struct NostrClient {
    /// The underlying nostr-sdk client
    client: Client,
    /// Whether the client has write access (has a secret key)
    has_write_access: bool,
    /// Public key of the user (if authenticated)
    public_key: Option<PublicKey>,
    /// Search-capable relays
    search_relays: Vec<String>,
    /// Connection state
    connected: Arc<RwLock<bool>>,
}

impl NostrClient {
    /// Creates a new Nostr client with the given configuration.
    ///
    /// # Arguments
    /// * `config` - The client configuration including secret key and relays
    ///
    /// # Returns
    /// A new `NostrClient` instance, or an error if initialization fails.
    pub async fn new(config: NostrClientConfig) -> Result<Self> {
        let (client, has_write_access, public_key) = if let Some(ref secret_key_str) = config.secret_key {
            // Parse the secret key (supports both nsec and hex formats)
            let keys = Self::parse_secret_key(secret_key_str)?;
            let public_key = keys.public_key();

            info!("Initialized with public key: {}", public_key.to_bech32()?);

            let client = Client::new(keys);
            (client, true, Some(public_key))
        } else {
            // Read-only mode: create client without keys
            let client = Client::default();
            (client, false, None)
        };

        // Add relays
        for relay_url in &config.relays {
            if let Err(e) = client.add_relay(relay_url).await {
                warn!("Failed to add relay {}: {}", relay_url, e);
            }
        }

        // Connect to relays
        client.connect().await;

        // Wait a moment for connections to establish
        tokio::time::sleep(Duration::from_millis(500)).await;

        Ok(Self {
            client,
            has_write_access,
            public_key,
            search_relays: config.search_relays,
            connected: Arc::new(RwLock::new(true)),
        })
    }

    /// Parses a secret key from nsec or hex format.
    fn parse_secret_key(secret_key_str: &str) -> Result<Keys> {
        let secret_key_str = secret_key_str.trim();

        let secret_key = if secret_key_str.starts_with("nsec") {
            SecretKey::from_bech32(secret_key_str)
                .context("Invalid nsec format")?
        } else {
            SecretKey::from_hex(secret_key_str)
                .context("Invalid hex secret key")?
        };

        Ok(Keys::new(secret_key))
    }

    /// Checks if the client has write access.
    #[allow(dead_code)]
    pub fn has_write_access(&self) -> bool {
        self.has_write_access
    }

    /// Gets the public key if authenticated.
    #[allow(dead_code)]
    pub fn public_key(&self) -> Option<PublicKey> {
        self.public_key
    }

    /// Posts a new note (Kind 1) with the given content.
    ///
    /// # Arguments
    /// * `content` - The text content of the note
    ///
    /// # Returns
    /// The event ID of the published note, or an error if publishing fails.
    pub async fn post_note(&self, content: &str) -> Result<EventId> {
        if !self.has_write_access {
            return Err(anyhow!("Cannot post notes in read-only mode. Please set NSEC or NOSTR_SECRET_KEY environment variable."));
        }

        let builder = EventBuilder::text_note(content);
        let output = self.client.send_event_builder(builder).await
            .context("Failed to publish note")?;

        let event_id = *output.id();
        info!("Published note with event ID: {}", event_id);
        Ok(event_id)
    }

    /// Gets the timeline (recent notes from followed users or global).
    ///
    /// # Arguments
    /// * `limit` - Maximum number of notes to retrieve
    ///
    /// # Returns
    /// A vector of notes with their metadata.
    pub async fn get_timeline(&self, limit: u64) -> Result<Vec<NoteInfo>> {
        let filter = if let Some(pk) = self.public_key {
            // Try to get contact list for personalized timeline
            let contact_filter = Filter::new()
                .author(pk)
                .kind(Kind::ContactList)
                .limit(1);

            let contacts = self.client
                .fetch_events(vec![contact_filter], Duration::from_secs(5))
                .await
                .ok()
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();

            if let Some(contact_event) = contacts.into_iter().next() {
                // Get followed public keys
                let followed: Vec<PublicKey> = contact_event.tags.iter()
                    .filter_map(|tag| {
                        if let Some(TagStandard::PublicKey { public_key, .. }) = tag.as_standardized() {
                            Some(*public_key)
                        } else {
                            None
                        }
                    })
                    .collect();

                if !followed.is_empty() {
                    debug!("Found {} followed accounts", followed.len());
                    Filter::new()
                        .authors(followed)
                        .kind(Kind::TextNote)
                        .limit(limit as usize)
                } else {
                    // Fallback to global timeline
                    Filter::new()
                        .kind(Kind::TextNote)
                        .limit(limit as usize)
                }
            } else {
                // No contact list, use global timeline
                Filter::new()
                    .kind(Kind::TextNote)
                    .limit(limit as usize)
            }
        } else {
            // Read-only mode: global timeline
            Filter::new()
                .kind(Kind::TextNote)
                .limit(limit as usize)
        };

        let events = self.client
            .fetch_events(vec![filter], Duration::from_secs(10))
            .await
            .context("Failed to fetch timeline")?;

        let mut notes: Vec<NoteInfo> = events
            .into_iter()
            .map(|event| NoteInfo {
                id: event.id.to_hex(),
                pubkey: event.pubkey.to_hex(),
                content: event.content.clone(),
                created_at: event.created_at.as_u64(),
            })
            .collect();

        // Sort by timestamp descending
        notes.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        notes.truncate(limit as usize);

        Ok(notes)
    }

    /// Searches for notes matching the given query using NIP-50 compatible relays.
    ///
    /// # Arguments
    /// * `query` - The search query string
    /// * `limit` - Maximum number of results to return
    ///
    /// # Returns
    /// A vector of matching notes.
    pub async fn search_notes(&self, query: &str, limit: u64) -> Result<Vec<NoteInfo>> {
        // Create a temporary client for search relays
        let search_client = Client::default();

        for relay_url in &self.search_relays {
            if let Err(e) = search_client.add_relay(relay_url).await {
                warn!("Failed to add search relay {}: {}", relay_url, e);
            }
        }

        search_client.connect().await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Create NIP-50 search filter
        let filter = Filter::new()
            .kind(Kind::TextNote)
            .search(query)
            .limit(limit as usize);

        let events = search_client
            .fetch_events(vec![filter], Duration::from_secs(15))
            .await
            .context("Failed to search notes")?;

        let mut notes: Vec<NoteInfo> = events
            .into_iter()
            .map(|event| NoteInfo {
                id: event.id.to_hex(),
                pubkey: event.pubkey.to_hex(),
                content: event.content.clone(),
                created_at: event.created_at.as_u64(),
            })
            .collect();

        // Sort by timestamp descending
        notes.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        notes.truncate(limit as usize);

        // Disconnect search client
        let _ = search_client.disconnect().await;

        Ok(notes)
    }

    /// Gets the profile information for a given user.
    ///
    /// # Arguments
    /// * `npub` - The user's public key in npub or hex format
    ///
    /// # Returns
    /// The user's profile information.
    pub async fn get_profile(&self, npub: &str) -> Result<ProfileInfo> {
        let npub = npub.trim();

        let public_key = if npub.starts_with("npub") {
            PublicKey::from_bech32(npub)
                .context("Invalid npub format")?
        } else {
            PublicKey::from_hex(npub)
                .context("Invalid hex public key")?
        };

        let filter = Filter::new()
            .author(public_key)
            .kind(Kind::Metadata)
            .limit(1);

        let events = self.client
            .fetch_events(vec![filter], Duration::from_secs(10))
            .await
            .context("Failed to fetch profile")?;

        let profile_event = events
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("Profile not found for {}", npub))?;

        // Parse the profile metadata JSON
        let metadata: Metadata = serde_json::from_str(&profile_event.content)
            .context("Failed to parse profile metadata")?;

        Ok(ProfileInfo {
            pubkey: public_key.to_hex(),
            npub: public_key.to_bech32()?,
            name: metadata.name,
            display_name: metadata.display_name,
            about: metadata.about,
            picture: metadata.picture,
            banner: metadata.banner,
            nip05: metadata.nip05,
            lud16: metadata.lud16,
            website: metadata.website,
        })
    }

    /// Disconnects from all relays.
    pub async fn disconnect(&self) {
        let _ = self.client.disconnect().await;
        let mut connected = self.connected.write().await;
        *connected = false;
    }
}

/// Information about a note.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NoteInfo {
    /// Event ID in hex format
    pub id: String,
    /// Author's public key in hex format
    pub pubkey: String,
    /// Note content
    pub content: String,
    /// Unix timestamp of creation
    pub created_at: u64,
}

/// Profile information for a user.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProfileInfo {
    /// Public key in hex format
    pub pubkey: String,
    /// Public key in npub format
    pub npub: String,
    /// Username
    pub name: Option<String>,
    /// Display name
    pub display_name: Option<String>,
    /// About/bio text
    pub about: Option<String>,
    /// Profile picture URL
    pub picture: Option<String>,
    /// Banner image URL
    pub banner: Option<String>,
    /// NIP-05 identifier
    pub nip05: Option<String>,
    /// Lightning address (LUD-16)
    pub lud16: Option<String>,
    /// Website URL
    pub website: Option<String>,
}
