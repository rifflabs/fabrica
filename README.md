# Palace Fabrica

**Coordination infrastructure for distributed teams**

A unified Discord bot that handles translation, status tracking, project visibility (Plane), and Git activity (GitHub).

## Features

### Translation
- Auto-translate non-English messages to English in channel
- DM English messages translated to subscribers' preferred language
- Per-channel enable/disable

### Status
- Self-reported availability: `/fabrica available`, `/fabrica busy`, `/fabrica away`
- Team visibility: `/fabrica who`, `/fabrica team`
- Time zone aware

### Plane Integration
- Project overview: `/fabrica project <name>`
- Issue listing: `/fabrica issues`
- Sprint status: `/fabrica sprint`

### GitHub Integration
- Repo status: `/fabrica repo <name>`
- Recent commits: `/fabrica commits <repo>`
- Open PRs: `/fabrica prs <repo>`

## Quick Start

```bash
# Build
cargo build --release

# Configure
cp fabrica.example.toml fabrica.toml
# Edit fabrica.toml with your tokens

# Run
export DISCORD_TOKEN="your-token"
./target/release/fabrica
```

## Commands

```
Translation:
  /fabrica translate subscribe <language>  - Get translations in your language
  /fabrica translate unsubscribe           - Stop getting translations
  /fabrica translate status                - Your translation settings
  /fabrica translate enable                - Enable in channel (admin)
  /fabrica translate disable               - Disable in channel (admin)

Status:
  /fabrica available [message]             - Mark as available
  /fabrica busy [message]                  - Mark as busy
  /fabrica away [message]                  - Mark as away
  /fabrica clear                           - Clear your status
  /fabrica who                             - Who's available now
  /fabrica team                            - Full team status

Shortcuts:
  /who                                     - Same as /fabrica who
  /team                                    - Same as /fabrica team
```

## Architecture

- **Rust** + **poise** (Discord framework)
- **SQLite** for persistence
- **Palace Translator** for LLM-based translation
- **axum** for webhook server

## License

AGPL-3.0-or-later
