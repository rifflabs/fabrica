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

-- Translation subscriptions (user wants DMs in their language)
CREATE TABLE IF NOT EXISTS translation_subscriptions (
    discord_id TEXT NOT NULL,
    language TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (discord_id, language)
);

-- Channels where translation is enabled
CREATE TABLE IF NOT EXISTS translation_channels (
    channel_id TEXT PRIMARY KEY,
    enabled_at INTEGER NOT NULL,
    enabled_by TEXT NOT NULL
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

-- Create indexes for common queries
CREATE INDEX IF NOT EXISTS idx_user_status_status ON user_status(status);
CREATE INDEX IF NOT EXISTS idx_translation_subs_language ON translation_subscriptions(language);
CREATE INDEX IF NOT EXISTS idx_github_watches_repo ON github_watches(repo);
CREATE INDEX IF NOT EXISTS idx_plane_watches_project ON plane_watches(project);
"#;
