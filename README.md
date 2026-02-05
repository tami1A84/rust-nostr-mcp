# Nostr MCP Server

A Model Context Protocol (MCP) server that enables AI agents like Claude to interact with the Nostr network.

## Overview

This server provides a bridge between AI assistants and the Nostr decentralized social network. It allows AI agents to:

- Post notes to Nostr
- Read timelines (personal or global)
- Search for notes
- Retrieve user profiles

## Installation

### Prerequisites

- Rust (latest stable version)
- A Nostr secret key (nsec) for write access (optional)

### Building from Source

```bash
git clone https://github.com/tami1A84/rust-nostr-mcp.git
cd rust-nostr-mcp
cargo build --release
```

The binary will be available at `target/release/nostr-mcp-server`.

## Configuration

### Environment Variables

Create a `.env` file in the project directory or set environment variables:

```bash
# Required for write access (posting notes)
# Use either NSEC or NOSTR_SECRET_KEY
NSEC=nsec1...

# Or in hex format
# NOSTR_SECRET_KEY=...

# Optional: Custom relay list (comma-separated)
NOSTR_RELAYS=wss://relay.damus.io,wss://nos.lol,wss://relay.nostr.band

# Optional: Search-capable relays for NIP-50 search (comma-separated)
NOSTR_SEARCH_RELAYS=wss://relay.nostr.band,wss://nostr.wine

# Optional: Logging level
RUST_LOG=info
```

### Default Relays

If not specified, the server uses these relays:

**General relays:**
- `wss://relay.damus.io`
- `wss://nos.lol`
- `wss://relay.nostr.band`
- `wss://nostr.wine`
- `wss://relay.snort.social`

**Search relays (NIP-50):**
- `wss://relay.nostr.band`
- `wss://nostr.wine`

## Usage

### Claude Desktop Configuration

Add the following to your Claude Desktop configuration file:

**macOS:** `~/Library/Application Support/Claude/claude_desktop_config.json`

**Windows:** `%APPDATA%\Claude\claude_desktop_config.json`

**Linux:** `~/.config/Claude/claude_desktop_config.json`

```json
{
  "mcpServers": {
    "nostr": {
      "command": "/path/to/nostr-mcp-server",
      "env": {
        "NSEC": "nsec1..."
      }
    }
  }
}
```

### Read-Only Mode

If no secret key is provided, the server runs in read-only mode. You can still:
- Get timelines (global)
- Search notes
- Get user profiles

But you cannot post notes.

## Available Tools

### 1. `post_note`

Post a new short text note (Kind 1) to the Nostr network.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `content` | string | Yes | The text content of the note to post |

**Example:**
```json
{
  "name": "post_note",
  "arguments": {
    "content": "Hello, Nostr!"
  }
}
```

### 2. `get_timeline`

Get the latest notes from the timeline. Returns notes from followed users if authenticated, otherwise returns the global timeline.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `limit` | integer | No | 20 | Maximum number of notes (1-100) |

**Example:**
```json
{
  "name": "get_timeline",
  "arguments": {
    "limit": 10
  }
}
```

### 3. `search_notes`

Search for notes containing specified keywords using NIP-50 search-capable relays.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `query` | string | Yes | - | The search query string |
| `limit` | integer | No | 20 | Maximum number of results (1-100) |

**Example:**
```json
{
  "name": "search_notes",
  "arguments": {
    "query": "bitcoin",
    "limit": 15
  }
}
```

### 4. `get_profile`

Get profile information for a Nostr user by their public key.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `npub` | string | Yes | Public key in npub (bech32) or hex format |

**Example:**
```json
{
  "name": "get_profile",
  "arguments": {
    "npub": "npub1..."
  }
}
```

## Use Cases

Here are some example prompts you can use with Claude:

- "What's happening on Nostr right now?" (get_timeline)
- "Post a note saying 'Good morning, Nostr!'" (post_note)
- "Search for discussions about Bitcoin on Nostr" (search_notes)
- "Who is npub1...? Get their profile." (get_profile)
- "Summarize today's news from Nostr and post a summary"
- "Post my daily report: [content]"

## Security Recommendations

### Recommended Usage Patterns

This MCP server supports different security levels depending on your needs:

#### General Users (This Implementation)

- **Best for:** Ease of use, local PC or server environments
- **Setup:** Store secret key in `.env` file
- **Use cases:**
  - "Summarize today's news from Nostr"
  - "Post my daily report"
  - Personal automation and AI assistance
- **Security:** The secret key is stored on the file system. Ensure proper file permissions (e.g., `chmod 600 .env`)

#### High-Security Users (Alternative Implementation)

- **Best for:** Users who don't want to store keys on the file system
- **Requires:** KeePassXC integration
- **Use cases:**
  - When you want human approval for every post
  - When AI should not have direct signing authority
  - Shared or multi-user environments
- **Setup:** Implement a separate MCP server that:
  1. Requests signatures from KeePassXC for each operation
  2. Requires user confirmation for every signing request
  3. Never stores the secret key on disk

### Best Practices

1. **Never commit your `.env` file** - Add it to `.gitignore`
2. **Use dedicated Nostr keys** - Don't use your main identity
3. **Review AI actions** - Monitor what the AI posts on your behalf
4. **Limit relay exposure** - Only connect to trusted relays
5. **Regular key rotation** - Consider rotating keys periodically

## Development

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run with logging
RUST_LOG=debug cargo run
```

### Testing the Server

You can test the server by sending JSON-RPC requests via stdin:

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | cargo run
```

### Project Structure

```
nostr-mcp-server/
├── Cargo.toml           # Project dependencies
├── README.md            # This file
├── LICENSE              # MIT License
└── src/
    ├── main.rs          # Entry point and configuration
    ├── mcp.rs           # MCP server implementation
    ├── nostr_client.rs  # Nostr SDK wrapper
    └── tools.rs         # Tool definitions and execution
```

## Dependencies

- [nostr-sdk](https://github.com/rust-nostr/nostr) - Nostr protocol implementation
- [tokio](https://tokio.rs/) - Async runtime
- [serde](https://serde.rs/) - Serialization/deserialization
- [dotenvy](https://github.com/allan2/dotenvy) - Environment variable loading
- [anyhow](https://github.com/dtolnay/anyhow) - Error handling
- [tracing](https://github.com/tokio-rs/tracing) - Logging

## Protocol

This server implements the Model Context Protocol (MCP) using JSON-RPC 2.0 over stdio. For more information about MCP, see the [MCP Specification](https://spec.modelcontextprotocol.io/).

## Troubleshooting

### "Cannot post notes in read-only mode"

Set the `NSEC` or `NOSTR_SECRET_KEY` environment variable with a valid Nostr secret key.

### "Profile not found"

The user may not have published their profile metadata (Kind 0 event), or the profile may not be available on the connected relays.

### Connection timeouts

Try adding more relays or checking your network connection. The server waits up to 10 seconds for relay responses.

### Search returns no results

Make sure the search relays support NIP-50. Not all relays implement search functionality.

## License

MIT License - See [LICENSE](LICENSE) for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Related Projects

- [nostr-sdk](https://github.com/rust-nostr/nostr) - The Nostr SDK for Rust
- [rust-nostr.org](https://rust-nostr.org/) - Documentation for rust-nostr

## Acknowledgments

- The [Nostr](https://nostr.com/) community
- [Anthropic](https://anthropic.com/) for Claude and the MCP specification
- Contributors to the rust-nostr ecosystem
