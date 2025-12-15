//! Database models for Palace Fabrica

use serde::{Deserialize, Serialize};

/// User status record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStatus {
    pub discord_id: String,
    pub status: String,
    pub message: Option<String>,
    pub updated_at: i64,
    pub timezone: Option<String>,
    pub preferred_hours_start: Option<String>,
    pub preferred_hours_end: Option<String>,
}

impl UserStatus {
    /// Create a new available status
    pub fn available(discord_id: impl Into<String>, message: Option<String>) -> Self {
        Self {
            discord_id: discord_id.into(),
            status: "available".to_string(),
            message,
            updated_at: chrono::Utc::now().timestamp(),
            timezone: None,
            preferred_hours_start: None,
            preferred_hours_end: None,
        }
    }

    /// Create a new busy status
    pub fn busy(discord_id: impl Into<String>, message: Option<String>) -> Self {
        Self {
            discord_id: discord_id.into(),
            status: "busy".to_string(),
            message,
            updated_at: chrono::Utc::now().timestamp(),
            timezone: None,
            preferred_hours_start: None,
            preferred_hours_end: None,
        }
    }

    /// Create a new away status
    pub fn away(discord_id: impl Into<String>, message: Option<String>) -> Self {
        Self {
            discord_id: discord_id.into(),
            status: "away".to_string(),
            message,
            updated_at: chrono::Utc::now().timestamp(),
            timezone: None,
            preferred_hours_start: None,
            preferred_hours_end: None,
        }
    }

    /// Get the emoji for this status
    pub fn emoji(&self) -> &'static str {
        match self.status.as_str() {
            "available" => "🟢",
            "busy" => "🟡",
            "away" => "🔴",
            _ => "⚫",
        }
    }

    /// Get a human-readable status label
    pub fn label(&self) -> &'static str {
        match self.status.as_str() {
            "available" => "Available",
            "busy" => "Busy",
            "away" => "Away",
            _ => "Unknown",
        }
    }
}

/// Translation subscription record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationSubscription {
    pub discord_id: String,
    pub language: String,
    pub created_at: i64,
}

/// User settings record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSettings {
    pub discord_id: String,
    pub timezone: String,
    pub time_format: String,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            discord_id: String::new(),
            timezone: "UTC".to_string(),
            time_format: "24h".to_string(),
        }
    }
}

impl UserSettings {
    pub fn new(discord_id: impl Into<String>) -> Self {
        Self {
            discord_id: discord_id.into(),
            ..Default::default()
        }
    }

    pub fn is_12h(&self) -> bool {
        self.time_format == "12h"
    }
}

/// Watch level for notifications
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WatchLevel {
    /// Every event
    All,
    /// PRs, releases, milestones
    Important,
    /// Only releases and merged PRs
    Minimal,
    /// Muted
    Off,
}

impl WatchLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            WatchLevel::All => "all",
            WatchLevel::Important => "important",
            WatchLevel::Minimal => "minimal",
            WatchLevel::Off => "off",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "all" => Some(WatchLevel::All),
            "important" => Some(WatchLevel::Important),
            "minimal" => Some(WatchLevel::Minimal),
            "off" => Some(WatchLevel::Off),
            _ => None,
        }
    }

    /// Check if this level should show a specific event type
    pub fn should_show(&self, event_type: &str) -> bool {
        match self {
            WatchLevel::Off => false,
            WatchLevel::Minimal => matches!(event_type, "release" | "pr_merged"),
            WatchLevel::Important => matches!(
                event_type,
                "release" | "pr_merged" | "pr_opened" | "pr_closed" | "milestone"
            ),
            WatchLevel::All => true,
        }
    }
}
