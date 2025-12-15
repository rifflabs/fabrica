//! Database schema for Palace Fabrica

pub const MIGRATIONS: &str = r#"
-- User status tracking
CREATE TABLE IF NOT EXISTS user_status (
    discord_id TEXT PRIMARY KEY,
    status TEXT NOT NULL CHECK (status IN ('available', 'busy', 'away')),
    message TEXT,
    updated_at INTEGER NOT NULL,
    timezone TEXT DEFAULT 'UTC',
    preferred_hours_start TEXT,
    preferred_hours_end TEXT
);

-- Translation subscriptions (user wants translations in their language, per channel per guild)
CREATE TABLE IF NOT EXISTS translation_subscriptions (
    guild_id TEXT NOT NULL,
    discord_id TEXT NOT NULL,
    channel_id TEXT NOT NULL,
    language TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    debug_mode INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (guild_id, discord_id, channel_id, language)
);

-- Channels where translation is enabled (per guild)
CREATE TABLE IF NOT EXISTS translation_channels (
    guild_id TEXT NOT NULL,
    channel_id TEXT NOT NULL,
    mode TEXT NOT NULL DEFAULT 'off' CHECK (mode IN ('off', 'silent', 'on', 'transparent')),
    enabled_at INTEGER NOT NULL,
    enabled_by TEXT NOT NULL,
    PRIMARY KEY (guild_id, channel_id)
);

-- Guild permissions (which roles can manage translation settings)
CREATE TABLE IF NOT EXISTS guild_permissions (
    guild_id TEXT NOT NULL,
    role_id TEXT NOT NULL,
    permission TEXT NOT NULL CHECK (permission IN ('mode', 'admin')),
    granted_at INTEGER NOT NULL,
    granted_by TEXT NOT NULL,
    PRIMARY KEY (guild_id, role_id, permission)
);

-- GitHub repository watches per channel
CREATE TABLE IF NOT EXISTS github_watches (
    channel_id TEXT NOT NULL,
    repo TEXT NOT NULL,
    level TEXT NOT NULL CHECK (level IN ('all', 'important', 'minimal', 'off')),
    PRIMARY KEY (channel_id, repo)
);

-- Plane project watches per channel
CREATE TABLE IF NOT EXISTS plane_watches (
    channel_id TEXT NOT NULL,
    project TEXT NOT NULL,
    level TEXT NOT NULL CHECK (level IN ('all', 'important', 'minimal', 'off')),
    PRIMARY KEY (channel_id, project)
);

-- Track when users last used /fabrica last command per channel
CREATE TABLE IF NOT EXISTS last_command_usage (
    guild_id TEXT NOT NULL,
    channel_id TEXT NOT NULL,
    discord_id TEXT NOT NULL,
    last_used_at INTEGER NOT NULL,
    last_message_id TEXT,
    PRIMARY KEY (guild_id, channel_id, discord_id)
);

-- Create indexes for tables that don't change
CREATE INDEX IF NOT EXISTS idx_user_status_status ON user_status(status);
CREATE INDEX IF NOT EXISTS idx_github_watches_repo ON github_watches(repo);
CREATE INDEX IF NOT EXISTS idx_plane_watches_project ON plane_watches(project);
"#;

/// Migration to add debug_mode column to existing databases
pub const MIGRATION_ADD_DEBUG_MODE: &str = r#"
ALTER TABLE translation_subscriptions ADD COLUMN debug_mode INTEGER NOT NULL DEFAULT 0;
"#;

/// Migration to add mode column to translation_channels
pub const MIGRATION_ADD_CHANNEL_MODE: &str = r#"
ALTER TABLE translation_channels ADD COLUMN mode TEXT NOT NULL DEFAULT 'on';
"#;

/// Migration to add channel_id to translation_subscriptions (recreates table)
pub const MIGRATION_ADD_CHANNEL_TO_SUBS: &str = r#"
ALTER TABLE translation_subscriptions ADD COLUMN channel_id TEXT NOT NULL DEFAULT '';
"#;

/// Migration to add guild_id to translation_subscriptions
pub const MIGRATION_ADD_GUILD_TO_SUBS: &str = r#"
ALTER TABLE translation_subscriptions ADD COLUMN guild_id TEXT NOT NULL DEFAULT '';
"#;

/// Migration to add guild_id to translation_channels
pub const MIGRATION_ADD_GUILD_TO_CHANNELS: &str = r#"
ALTER TABLE translation_channels ADD COLUMN guild_id TEXT NOT NULL DEFAULT '';
"#;

/// Migration to create indexes on guild_id columns (run after ALTER TABLE migrations)
pub const MIGRATION_CREATE_GUILD_INDEXES: &str = r#"
CREATE INDEX IF NOT EXISTS idx_translation_subs_guild ON translation_subscriptions(guild_id);
CREATE INDEX IF NOT EXISTS idx_translation_subs_channel ON translation_subscriptions(channel_id);
CREATE INDEX IF NOT EXISTS idx_translation_channels_guild ON translation_channels(guild_id);
CREATE INDEX IF NOT EXISTS idx_guild_permissions_guild ON guild_permissions(guild_id);
CREATE INDEX IF NOT EXISTS idx_last_command_usage_user ON last_command_usage(discord_id);
"#;

/// Migration to fix translation_channels primary key to include guild_id
/// This properly migrates the table by creating a new one, copying data, and replacing
pub const MIGRATION_FIX_TRANSLATION_CHANNELS_PK: &str = r#"
CREATE TABLE IF NOT EXISTS translation_channels_new (
    guild_id TEXT NOT NULL,
    channel_id TEXT NOT NULL,
    mode TEXT NOT NULL DEFAULT 'off' CHECK (mode IN ('off', 'silent', 'on', 'transparent')),
    enabled_at INTEGER NOT NULL,
    enabled_by TEXT NOT NULL,
    PRIMARY KEY (guild_id, channel_id)
);
INSERT OR IGNORE INTO translation_channels_new (guild_id, channel_id, mode, enabled_at, enabled_by)
    SELECT COALESCE(guild_id, ''), channel_id, COALESCE(mode, 'off'), enabled_at, enabled_by
    FROM translation_channels;
DROP TABLE translation_channels;
ALTER TABLE translation_channels_new RENAME TO translation_channels;
"#;

/// Migration to fix translation_subscriptions primary key to include guild_id and channel_id
pub const MIGRATION_FIX_TRANSLATION_SUBS_PK: &str = r#"
CREATE TABLE IF NOT EXISTS translation_subscriptions_new (
    guild_id TEXT NOT NULL,
    discord_id TEXT NOT NULL,
    channel_id TEXT NOT NULL,
    language TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    debug_mode INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (guild_id, discord_id, channel_id, language)
);
INSERT OR IGNORE INTO translation_subscriptions_new (guild_id, discord_id, channel_id, language, created_at, debug_mode)
    SELECT COALESCE(guild_id, ''), discord_id, COALESCE(channel_id, ''), language, created_at, COALESCE(debug_mode, 0)
    FROM translation_subscriptions;
DROP TABLE translation_subscriptions;
ALTER TABLE translation_subscriptions_new RENAME TO translation_subscriptions;
"#;

/// Migration to add user schedule tables (per-guild)
pub const MIGRATION_ADD_USER_SCHEDULES: &str = r#"
-- Weekly recurring schedule (e.g., Mon-Fri 9:30-23:30) per guild
CREATE TABLE IF NOT EXISTS user_weekly_schedule (
    guild_id TEXT NOT NULL,
    discord_id TEXT NOT NULL,
    day_of_week INTEGER NOT NULL CHECK (day_of_week >= 0 AND day_of_week <= 6),
    start_time TEXT NOT NULL,
    end_time TEXT NOT NULL,
    PRIMARY KEY (guild_id, discord_id, day_of_week)
);

-- One-off schedule overrides (e.g., "today until 23:30") per guild
CREATE TABLE IF NOT EXISTS user_schedule_override (
    guild_id TEXT NOT NULL,
    discord_id TEXT NOT NULL,
    date TEXT NOT NULL,
    start_time TEXT,
    end_time TEXT NOT NULL,
    PRIMARY KEY (guild_id, discord_id, date)
);

CREATE INDEX IF NOT EXISTS idx_user_weekly_schedule_guild ON user_weekly_schedule(guild_id);
CREATE INDEX IF NOT EXISTS idx_user_weekly_schedule_user ON user_weekly_schedule(discord_id);
CREATE INDEX IF NOT EXISTS idx_user_schedule_override_guild ON user_schedule_override(guild_id);
CREATE INDEX IF NOT EXISTS idx_user_schedule_override_user ON user_schedule_override(discord_id);
CREATE INDEX IF NOT EXISTS idx_user_schedule_override_date ON user_schedule_override(date);
"#;

/// Migration to fix user_weekly_schedule table to include guild_id
pub const MIGRATION_FIX_USER_WEEKLY_SCHEDULE_PK: &str = r#"
CREATE TABLE IF NOT EXISTS user_weekly_schedule_new (
    guild_id TEXT NOT NULL,
    discord_id TEXT NOT NULL,
    day_of_week INTEGER NOT NULL CHECK (day_of_week >= 0 AND day_of_week <= 6),
    start_time TEXT NOT NULL,
    end_time TEXT NOT NULL,
    PRIMARY KEY (guild_id, discord_id, day_of_week)
);
INSERT OR IGNORE INTO user_weekly_schedule_new (guild_id, discord_id, day_of_week, start_time, end_time)
    SELECT '', discord_id, day_of_week, start_time, end_time
    FROM user_weekly_schedule;
DROP TABLE user_weekly_schedule;
ALTER TABLE user_weekly_schedule_new RENAME TO user_weekly_schedule;
"#;

/// Migration to add user_settings table
pub const MIGRATION_ADD_USER_SETTINGS: &str = r#"
CREATE TABLE IF NOT EXISTS user_settings (
    discord_id TEXT PRIMARY KEY,
    timezone TEXT DEFAULT 'UTC',
    time_format TEXT DEFAULT '24h' CHECK (time_format IN ('24h', '12h'))
);
"#;

/// Migration to add always_show_me column to user_settings
pub const MIGRATION_ADD_ALWAYS_SHOW_ME: &str = r#"
ALTER TABLE user_settings ADD COLUMN always_show_me INTEGER NOT NULL DEFAULT 0;
"#;

/// Migration to add user dialect preferences table
pub const MIGRATION_ADD_DIALECT_PREFERENCES: &str = r#"
CREATE TABLE IF NOT EXISTS user_dialect_preferences (
    discord_id TEXT NOT NULL,
    language TEXT NOT NULL,
    dialect TEXT NOT NULL,
    PRIMARY KEY (discord_id, language)
);
CREATE INDEX IF NOT EXISTS idx_dialect_prefs_user ON user_dialect_preferences(discord_id);
"#;

/// Migration to fix user_schedule_override table to include guild_id
pub const MIGRATION_FIX_USER_SCHEDULE_OVERRIDE_PK: &str = r#"
CREATE TABLE IF NOT EXISTS user_schedule_override_new (
    guild_id TEXT NOT NULL,
    discord_id TEXT NOT NULL,
    date TEXT NOT NULL,
    start_time TEXT,
    end_time TEXT NOT NULL,
    PRIMARY KEY (guild_id, discord_id, date)
);
INSERT OR IGNORE INTO user_schedule_override_new (guild_id, discord_id, date, start_time, end_time)
    SELECT '', discord_id, date, start_time, end_time
    FROM user_schedule_override;
DROP TABLE user_schedule_override;
ALTER TABLE user_schedule_override_new RENAME TO user_schedule_override;
"#;

/// Migration to add default_language column to user_settings
pub const MIGRATION_ADD_DEFAULT_LANGUAGE: &str = r#"
ALTER TABLE user_settings ADD COLUMN default_language TEXT;
"#;
