# Palace Fabrica User Guide

Fabrica is a Discord bot for team coordination - tracking availability, working hours, and providing real-time translation.

## Quick Start

All commands start with `/fabrica`.

## Status Commands

### Set Your Availability

```
/fabrica available [message]    # Mark yourself as available
/fabrica busy [message]         # Mark yourself as busy
/fabrica away [message]         # Mark yourself as away
/fabrica clear                  # Clear your status
```

The optional message lets others know what you're working on.

### View Team Status

```
/fabrica who                    # Quick view of available/busy members
/fabrica team                   # Shows available members (only you see this)
/fabrica team public            # Posts to channel for everyone
```

## Working Hours

Set your working hours so teammates know when you're available.

### Set Weekly Schedule

```
/fabrica hours M-F 9am to 5pm           # Monday-Friday
/fabrica hours Mon,Wed,Fri 10:00 to 18:00
/fabrica hours Tue 14:00 to 22:00       # Override just Tuesday
```

### Set Today's Hours

```
/fabrica hours today 9am to 5pm         # Full range for today
/fabrica hours today until 5pm          # Available until 5pm
/fabrica hours until 11pm               # Shorthand for "today until"
```

### View Your Schedule

```
/fabrica hours                          # Shows your current schedule
```

### Time Formats

Both 12-hour and 24-hour formats are supported:

| Format | Example |
|--------|---------|
| 24-hour | `17:30`, `9:00` |
| 24-hour short | `17` (means 17:00) |
| 12-hour | `5pm`, `9am` |
| 12-hour with minutes | `5:30pm` |
| 12-hour with space | `5 pm` |

## User Settings

Customize how times are displayed to you.

### View Settings

```
/fabrica settings
```

### Set Timezone

```
/fabrica settings timezone London
/fabrica settings timezone America/New_York
/fabrica settings timezone Tokyo
```

Common aliases are supported: `NYC`, `LA`, `EST`, `PST`, `GMT`, `IST`, etc.

### Set Time Format

```
/fabrica settings format 24h            # 17:30
/fabrica settings format 12h            # 5:30pm
```

## Translation

Fabrica can translate messages in real-time and DM translations to subscribers.

### Subscribe to Translations

```
/fabrica translate subscribe en         # Receive English translations
/fabrica translate subscribe hi         # Receive Hindi translations
/fabrica translate subscribe fr         # Receive French translations
```

### Unsubscribe

```
/fabrica translate unsubscribe en       # Stop English translations
/fabrica translate unsubscribe all      # Stop all translations
```

### View Your Subscriptions

```
/fabrica translate status
```

### Channel Translation Modes

Admins can set how translation works in each channel:

```
/fabrica translate mode off             # Disabled
/fabrica translate mode silent          # DMs only, no channel indicator
/fabrica translate mode on              # DMs + channel reaction indicator
/fabrica translate mode transparent     # Full visibility (shows translations)
```

### Debug Mode

Test translations by receiving your own messages:

```
/fabrica translate debug                # Toggle debug mode
```

### Catch Up

See recent messages translated:

```
/fabrica translate last                 # Last 10 messages
/fabrica translate last 50              # Last 50 messages
/fabrica last 20                        # Shortcut
```

## Server Administration

Server admins can configure permissions for who can manage Fabrica.

### View Server Status

```
/fabrica server status
/fabrica server permissions
```

### Grant Permissions

```
/fabrica server allow mode @role        # Role can change translation modes
/fabrica server allow admin @role       # Role can manage all settings
/fabrica server allow mode everyone     # Everyone can change modes
```

### Revoke Permissions

```
/fabrica server deny mode @role
/fabrica server deny admin @role
/fabrica server deny mode everyone
```

### Permission Types

- **mode** - Can change translation modes in channels
- **admin** - Can manage all Fabrica settings for the server

### Admin Timezone Override

Global admins (configured in `fabrica.toml`) can set timezones for other users:

```
/fabrica settings timezone London @user
```

## Configuration

Fabrica is configured via `fabrica.toml`. Key settings:

```toml
[discord]
token = "${DISCORD_TOKEN}"
guild_ids = ["123456789", "987654321"]  # Servers where bot operates
admin_ids = ["111222333"]                # Global admin user IDs

[translation]
backend = "openrouter"
openrouter_api_key = "${OPENROUTER_API_KEY}"
model = "mistralai/mistral-small-3.1-24b-instruct"
```

## Tips

1. **Hours are per-server** - You can have different schedules in different Discord servers
2. **Team is ephemeral** - `/fabrica team` only shows to you; use `public` to share
3. **Times respect your settings** - Set your timezone and format preference once
4. **Translations are per-channel** - Subscribe in each channel you want translations
