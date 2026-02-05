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
