# Palace Fabrica - Coordination Infrastructure for Riff Labs

**Version:** 1.0.0
**Date:** 2025-12-15
**Author:** Lief (The Forge)
**Status:** Specification

---

## Executive Summary

Palace Fabrica is a unified Discord bot that removes Wings as the coordination bottleneck for Riff Labs. It handles translation, status/availability, project visibility (Plane), and Git activity (GitHub) - all in one cohesive system.

**The Problem:** ~200 hours/week of staff capacity sitting idle because:
- Wings is the only translator (Hindi ↔ English)
- Wings is the only one who knows who's available
- Wings is the only one who knows project status
- Wings is the only conduit for direction

**The Solution:** Fabrica makes coordination self-service. Anyone can see who's available, communicate across language barriers, and understand project state without asking Wings.

---

## Core Philosophy

### 1. Coordination, NOT Surveillance
Fabrica exists to help people work together, not to monitor them. Status is self-reported, optional, and fully under each person's control.

### 2. Painfully Easy UX
People WILL forget to toggle status. People WILL forget commands. Design for humans who are busy building things, not administering bots.

### 3. Graceful Degradation
If translation fails, show the original. If Plane is down, say so. Never block work because a feature is broken.

### 4. One Bot, All Duties
No separate bots for each function. One unified interface, one mental model, one `/fabrica` namespace.

---

## Modules

### Module 1: Translation

**Purpose:** Remove the language barrier blocking 76 hours/week (Muskan + Preeti)

**How It Works:**

```
User posts message in #general (translation-enabled channel)
    │
    ▼
Fabrica detects language automatically
    │
    ├─► If NOT English (e.g., Hindi):
    │       │
    │       ▼
    │   Translate to English
    │       │
    │       ▼
    │   Post in channel: "🌐 [Translation] <english text>"
    │   (Original message stays visible above)
    │
    └─► If English:
            │
            ▼
        Query: Who subscribes to translations in this channel?
            │
            ▼
        For each subscriber (e.g., Hindi speakers):
            DM: "[#general] @username said: <hindi translation>"
```

**Key Design Decisions:**

1. **Non-English → Channel**: Public translations help everyone follow along
2. **English → DM**: Doesn't clutter channel, respects that most speak English
3. **Per-channel opt-in**: Not every channel needs translation
4. **Per-user subscriptions**: Each person controls what they receive
5. **Language detection**: Automatic, no need to tag messages

**Commands:**

```
/fabrica translate subscribe <language>
    Subscribe to receive translations in your preferred language
    Example: /fabrica translate subscribe hindi

/fabrica translate unsubscribe
    Stop receiving translation DMs

/fabrica translate status
    Show your current translation settings

/fabrica translate enable (admin only)
    Enable translation in the current channel

/fabrica translate disable (admin only)
    Disable translation in the current channel
```

**Translation Backend:**

Uses Palace Translator to route to cost-effective LLMs. Translation is a perfect use case for smaller models - Mistral/Devstral handles this well at a fraction of Claude's cost.

```rust
pub struct TranslationService {
    palace_url: String,  // http://localhost:19848
    model: String,       // "mistral" or "devstral"
}

impl TranslationService {
    pub async fn translate(&self, text: &str, from: &str, to: &str) -> Result<String> {
        // Route through Palace Translator
        // Prompt: "Translate the following {from} text to {to}.
        //          Output ONLY the translation, nothing else: {text}"
    }
}
```

---

### Module 2: Status/Availability

**Purpose:** Coordination visibility without surveillance

**States:**

| State | Emoji | Meaning |
|-------|-------|---------|
| `available` | 🟢 | Ready to work/collaborate |
| `busy` | 🟡 | Working, prefer not to interrupt |
| `away` | 🔴 | Not actively working |
| (none) | ⚫ | No status set |

**Commands:**

```
/fabrica available [what]
    Mark yourself as available
    Example: /fabrica available "ready for code review"

/fabrica busy [what]
    Mark yourself as busy/focused
    Example: /fabrica busy "deep in SPIRAL implementation"

/fabrica away [reason]
    Mark yourself as away
    Example: /fabrica away "back in 2 hours"

/fabrica clear
    Clear your status

/fabrica who
    Show who's currently available

/fabrica team
    Show full team status with time zones
```

**Display Example:**

```
/fabrica who
───────────────────────────────
🟢 Available (3)
  @wings - Working on Fabrica spec
  @ben - Exploring codebase
  @ky - Ready for tasks

🟡 Busy (1)
  @julien - SPIRAL implementation

🔴 Away (2)
  @muskan - Back at 14:00 IST
  @preeti - Tomorrow
───────────────────────────────
```

**Time Zone Awareness:**

```
/fabrica team
───────────────────────────────
Team Status (2025-12-15)

🇨🇦 Pacific (08:30)
  🟢 @wings - Fabrica spec

🇫🇷 Paris (17:30)
  🟡 @julien - SPIRAL

🇮🇳 India (22:00)
  🔴 @muskan - Tomorrow
  🔴 @preeti - Tomorrow

🇺🇸 Eastern (11:30)
  🟢 @ben - Onboarding
───────────────────────────────
```

**Persistence:**

Status persists across bot restarts. Stored in SQLite.

```sql
CREATE TABLE user_status (
    discord_id TEXT PRIMARY KEY,
    status TEXT NOT NULL,           -- available, busy, away
    message TEXT,                   -- what they're working on
    updated_at TIMESTAMP NOT NULL,
    timezone TEXT,                  -- e.g., "America/Vancouver"
    preferred_hours_start TEXT,     -- e.g., "09:00"
    preferred_hours_end TEXT        -- e.g., "17:00"
);
```

**UX Consideration: Forgetting to Toggle**

People will forget. Options to help:
1. Auto-clear status after 8 hours of no update
2. Daily reminder DM: "You're still marked as away - is that right?"
3. Discord presence integration (if they're offline in Discord, mark away)

---

### Module 3: Plane Integration

**Purpose:** Project visibility at a glance

**Reuse:** We have a working Plane MCP server at `/mnt/castle/workspace/plane-mcp-server`. Fabrica reuses that client logic.

**Commands:**

```
/fabrica project <name>
    Show project overview
    Example: /fabrica project flagship

/fabrica issues [project] [status]
    List issues, optionally filtered
    Example: /fabrica issues flagship open

/fabrica sprint [project]
    Show current sprint status
    Example: /fabrica sprint citadel

/fabrica assign <issue-id> <person>
    Assign an issue to someone
    Example: /fabrica assign CITADEL-42 @julien
```

**Display Example:**

```
/fabrica project flagship
───────────────────────────────
📊 Flagship

Status: Active
Sprint: December Sprint 2
Progress: ████████░░ 78%

Open Issues: 12
  🔴 Critical: 1
  🟡 High: 3
  🟢 Medium: 8

Recent Activity:
  • FLAGSHIP-89 merged (2h ago)
  • FLAGSHIP-91 opened by @ben (4h ago)
───────────────────────────────
```

**Webhook Integration:**

Fabrica listens for Plane webhooks and posts to configured channels:

```
[#flagship-dev]
📋 New Issue: FLAGSHIP-92
"Fix mobile layout on artist page"
Assigned: @muskan
Priority: Medium
```

**Channel Configuration:**

```
/fabrica watch plane <project> [level]
    Watch a Plane project in this channel
    Levels: all, important, minimal

/fabrica unwatch plane <project>
    Stop watching a project
```

---

### Module 4: GitHub Integration

**Purpose:** Git activity visibility

**Commands:**

```
/fabrica repo <name>
    Show repository status
    Example: /fabrica repo citadel

/fabrica commits <repo> [count]
    Show recent commits
    Example: /fabrica commits citadel 5

/fabrica prs <repo>
    Show open pull requests
    Example: /fabrica prs flagship
```

**Webhook Events:**

| Event | Action |
|-------|--------|
| `push` | Post commit summary to channel |
| `pull_request.opened` | Announce new PR |
| `pull_request.merged` | Celebrate merge |
| `pull_request.closed` | Note closure |
| `issues.opened` | Announce new issue |
| `release.published` | Announce release |

**Activity Levels:**

```
/fabrica watch github <repo> [level]
    Watch a GitHub repo in this channel

    Levels:
    - all: Every push, PR, issue, comment
    - important: PRs, releases, milestones (default)
    - minimal: Only releases and merged PRs
    - off: Muted
```

**Display Example:**

```
[#citadel-dev]
🔀 PR Merged: citadel#47
"Fix TGP Q proof flooding"
by @wings • 8 files changed

───────────────────────────────

[#releases]
🚀 New Release: flagship v0.8.0
"Performance improvements and bug fixes"
• 66ms cold load time achieved
• Fixed mobile layout issues
• Added artist analytics
```

---

## Architecture

### Tech Stack

- **Language:** Rust (per Palace guidelines)
- **Discord Framework:** `poise` (modern, built on serenity)
- **Database:** SQLite (simple, file-based, sufficient for MVP)
- **HTTP Server:** `axum` (for webhooks)
- **Translation:** Palace Translator → Mistral/Devstral
- **Plane Client:** Extracted from plane-mcp-server
- **GitHub Client:** `octocrab` crate

### Project Structure

```
fabrica/
├── Cargo.toml
├── SPEC.md                    # This file
├── README.md                  # Quick start guide
├── fabrica.toml               # Configuration
├── src/
│   ├── main.rs                # Entry point
│   ├── lib.rs                 # Library exports
│   ├── config.rs              # Configuration loading
│   ├── bot.rs                 # Discord bot setup
│   ├── db/
│   │   ├── mod.rs
│   │   ├── schema.rs          # SQLite schema
│   │   └── models.rs          # Data models
│   ├── modules/
│   │   ├── mod.rs
│   │   ├── translation.rs     # Translation commands & events
│   │   ├── status.rs          # Status commands
│   │   ├── plane.rs           # Plane commands & webhooks
│   │   └── github.rs          # GitHub commands & webhooks
│   ├── services/
│   │   ├── mod.rs
│   │   ├── translator.rs      # Palace Translator client
│   │   ├── plane_client.rs    # Plane API client
│   │   └── github_client.rs   # GitHub API client
│   ├── webhooks/
│   │   ├── mod.rs
│   │   ├── server.rs          # Axum webhook server
│   │   ├── plane.rs           # Plane webhook handlers
│   │   └── github.rs          # GitHub webhook handlers
│   └── util/
│       ├── mod.rs
│       └── language.rs        # Language detection
├── migrations/
│   └── 001_initial.sql        # Initial schema
└── tests/
    ├── translation_test.rs
    ├── status_test.rs
    └── integration/
```

### Configuration

```toml
# fabrica.toml

[discord]
token = "${DISCORD_TOKEN}"
application_id = 123456789
guild_id = 987654321  # Riff Labs server

[database]
path = "fabrica.db"

[translation]
backend = "palace"
palace_url = "http://localhost:19848"
model = "mistral"
default_language = "en"
supported_languages = ["en", "hi", "fr"]  # English, Hindi, French

[plane]
url = "https://plane.riff.cc"
api_key = "${PLANE_API_KEY}"
workspace = "riff"

[github]
token = "${GITHUB_TOKEN}"
webhook_secret = "${GITHUB_WEBHOOK_SECRET}"
org = "riffcc"

[webhooks]
host = "0.0.0.0"
port = 8080
base_url = "https://fabrica.riff.cc"  # For webhook registration
```

### Database Schema

```sql
-- migrations/001_initial.sql

-- User status tracking
CREATE TABLE user_status (
    discord_id TEXT PRIMARY KEY,
    status TEXT NOT NULL CHECK (status IN ('available', 'busy', 'away')),
    message TEXT,
    updated_at INTEGER NOT NULL,  -- Unix timestamp
    timezone TEXT DEFAULT 'UTC',
    preferred_hours_start TEXT,
    preferred_hours_end TEXT
);

-- Translation subscriptions
CREATE TABLE translation_subscriptions (
    discord_id TEXT NOT NULL,
    language TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (discord_id, language)
);

-- Translation-enabled channels
CREATE TABLE translation_channels (
    channel_id TEXT PRIMARY KEY,
    enabled_at INTEGER NOT NULL,
    enabled_by TEXT NOT NULL
);

-- GitHub watch configurations
CREATE TABLE github_watches (
    channel_id TEXT NOT NULL,
    repo TEXT NOT NULL,
    level TEXT NOT NULL CHECK (level IN ('all', 'important', 'minimal', 'off')),
    PRIMARY KEY (channel_id, repo)
);

-- Plane watch configurations
CREATE TABLE plane_watches (
    channel_id TEXT NOT NULL,
    project TEXT NOT NULL,
    level TEXT NOT NULL CHECK (level IN ('all', 'important', 'minimal', 'off')),
    PRIMARY KEY (channel_id, project)
);
```

---

## Implementation Priority

### Phase 1: Translation (Week 1) - HIGHEST IMPACT
- [ ] Language detection
- [ ] Palace Translator integration
- [ ] Non-English → channel translation
- [ ] English → DM translation
- [ ] Subscription management
- [ ] Channel enable/disable

**Unblocks:** 76 hours/week (Muskan + Preeti)

### Phase 2: Status (Week 1-2)
- [ ] Status commands (available/busy/away/clear)
- [ ] /who and /team commands
- [ ] Time zone support
- [ ] Status persistence
- [ ] Auto-clear stale status

**Unblocks:** Async coordination without Wings

### Phase 3: Plane Integration (Week 2)
- [ ] Port plane-mcp-server client
- [ ] Project/issues/sprint commands
- [ ] Webhook receiver
- [ ] Channel notifications
- [ ] Watch configuration

**Unblocks:** Project visibility without Wings

### Phase 4: GitHub Integration (Week 2-3)
- [ ] GitHub client setup
- [ ] Repo/commits/prs commands
- [ ] Webhook receiver
- [ ] Channel notifications
- [ ] Watch configuration

**Unblocks:** Git activity visibility

---

## Security Considerations

1. **Tokens in Environment:** Discord token, API keys never in config files
2. **Webhook Verification:** Validate GitHub webhook signatures
3. **Rate Limiting:** Prevent command spam
4. **No Message Storage:** Translation is ephemeral, not logged
5. **User Data Control:** Users can delete their data anytime

---

## Error Handling

| Situation | Behavior |
|-----------|----------|
| Translation fails | Post original with ⚠️, log error |
| Plane API down | "Plane is currently unavailable" |
| GitHub API down | "GitHub is currently unavailable" |
| Database error | Log, continue (status is non-critical) |
| Unknown language | Skip translation, no error shown |
| Rate limited | Queue and retry with backoff |

---

## Future Considerations

Not in MVP, but valuable later:

1. **Standup Bot:** Daily async standups - "What did you do? What's blocked?"
2. **Voice Translation:** Real-time voice channel translation
3. **AI Summaries:** "What happened in #citadel-dev this week?"
4. **Time Tracking:** Opt-in billable hours tracking
5. **Meeting Scheduler:** Find overlapping availability across time zones
6. **Slack Bridge:** If we ever need Slack compatibility

---

## Success Metrics

**Week 1:**
- [ ] Translation working in at least one channel
- [ ] Muskan and Preeti participating in English discussions
- [ ] Status commands functional

**Week 2:**
- [ ] All team members using status
- [ ] Plane integration showing project state
- [ ] At least 50% reduction in "is X available?" questions

**Month 1:**
- [ ] Full adoption across team
- [ ] Wings spending <10% of time on coordination
- [ ] 200 hours/week actually productive

---

## Command Reference

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

Plane:
  /fabrica project <name>                  - Project overview
  /fabrica issues [project] [status]       - List issues
  /fabrica sprint [project]                - Sprint status
  /fabrica watch plane <project> [level]   - Watch project here
  /fabrica unwatch plane <project>         - Stop watching

GitHub:
  /fabrica repo <name>                     - Repo overview
  /fabrica commits <repo> [count]          - Recent commits
  /fabrica prs <repo>                      - Open PRs
  /fabrica watch github <repo> [level]     - Watch repo here
  /fabrica unwatch github <repo>           - Stop watching
```

---

## Quick Start (For Developers)

```bash
# Clone and build
cd /mnt/riffcastle/lagun-project/fabrica
cargo build --release

# Configure
cp fabrica.example.toml fabrica.toml
# Edit fabrica.toml with your tokens

# Run
export DISCORD_TOKEN="your-token"
export PLANE_API_KEY="your-key"
export GITHUB_TOKEN="your-token"
./target/release/fabrica

# Or with cargo
cargo run --release
```

---

*e cinere surgemus*

**Palace Fabrica** - From ashes, we coordinate.
