//! Database layer for Palace Fabrica

mod models;
mod schema;

pub use models::*;

use anyhow::Result;
use std::sync::Arc;
use tokio_rusqlite::Connection;
use tracing::info;

/// Database handle for Fabrica
#[derive(Clone, Debug)]
pub struct Database {
    conn: Arc<Connection>,
}

impl Database {
    /// Create a new database connection
    pub async fn new(path: &str) -> Result<Self> {
        let conn = Connection::open(path).await?;
        Ok(Self {
            conn: Arc::new(conn),
        })
    }

    /// Run database migrations
    pub async fn migrate(&self) -> Result<()> {
        self.conn
            .call(|conn| {
                conn.execute_batch(schema::MIGRATIONS)?;
                Ok(())
            })
            .await?;
        info!("Database migrations complete");
        Ok(())
    }

    // ==================== User Status ====================

    /// Get a user's status
    pub async fn get_status(&self, discord_id: &str) -> Result<Option<UserStatus>> {
        let id = discord_id.to_string();
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT discord_id, status, message, updated_at, timezone,
                            preferred_hours_start, preferred_hours_end
                     FROM user_status WHERE discord_id = ?",
                )?;
                let result = stmt
                    .query_row([&id], |row| {
                        Ok(UserStatus {
                            discord_id: row.get(0)?,
                            status: row.get(1)?,
                            message: row.get(2)?,
                            updated_at: row.get(3)?,
                            timezone: row.get(4)?,
                            preferred_hours_start: row.get(5)?,
                            preferred_hours_end: row.get(6)?,
                        })
                    })
                    .optional()?;
                Ok(result)
            })
            .await
            .map_err(Into::into)
    }

    /// Set a user's status
    pub async fn set_status(&self, status: UserStatus) -> Result<()> {
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT OR REPLACE INTO user_status
                     (discord_id, status, message, updated_at, timezone,
                      preferred_hours_start, preferred_hours_end)
                     VALUES (?, ?, ?, ?, ?, ?, ?)",
                    rusqlite::params![
                        status.discord_id,
                        status.status,
                        status.message,
                        status.updated_at,
                        status.timezone,
                        status.preferred_hours_start,
                        status.preferred_hours_end,
                    ],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    /// Clear a user's status
    pub async fn clear_status(&self, discord_id: &str) -> Result<()> {
        let id = discord_id.to_string();
        self.conn
            .call(move |conn| {
                conn.execute("DELETE FROM user_status WHERE discord_id = ?", [&id])?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    /// Get all users with a specific status
    pub async fn get_users_by_status(&self, status: &str) -> Result<Vec<UserStatus>> {
        let status = status.to_string();
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT discord_id, status, message, updated_at, timezone,
                            preferred_hours_start, preferred_hours_end
                     FROM user_status WHERE status = ?",
                )?;
                let rows = stmt
                    .query_map([&status], |row| {
                        Ok(UserStatus {
                            discord_id: row.get(0)?,
                            status: row.get(1)?,
                            message: row.get(2)?,
                            updated_at: row.get(3)?,
                            timezone: row.get(4)?,
                            preferred_hours_start: row.get(5)?,
                            preferred_hours_end: row.get(6)?,
                        })
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .await
            .map_err(Into::into)
    }

    /// Get all user statuses
    pub async fn get_all_statuses(&self) -> Result<Vec<UserStatus>> {
        self.conn
            .call(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT discord_id, status, message, updated_at, timezone,
                            preferred_hours_start, preferred_hours_end
                     FROM user_status ORDER BY status, updated_at DESC",
                )?;
                let rows = stmt
                    .query_map([], |row| {
                        Ok(UserStatus {
                            discord_id: row.get(0)?,
                            status: row.get(1)?,
                            message: row.get(2)?,
                            updated_at: row.get(3)?,
                            timezone: row.get(4)?,
                            preferred_hours_start: row.get(5)?,
                            preferred_hours_end: row.get(6)?,
                        })
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .await
            .map_err(Into::into)
    }

    // ==================== Translation ====================

    /// Add a translation subscription
    pub async fn add_translation_subscription(&self, discord_id: &str, language: &str) -> Result<()> {
        let id = discord_id.to_string();
        let lang = language.to_string();
        let now = chrono::Utc::now().timestamp();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT OR REPLACE INTO translation_subscriptions
                     (discord_id, language, created_at) VALUES (?, ?, ?)",
                    rusqlite::params![id, lang, now],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    /// Remove a translation subscription
    pub async fn remove_translation_subscription(&self, discord_id: &str) -> Result<()> {
        let id = discord_id.to_string();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "DELETE FROM translation_subscriptions WHERE discord_id = ?",
                    [&id],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    /// Get a user's translation subscription
    pub async fn get_translation_subscription(&self, discord_id: &str) -> Result<Option<String>> {
        let id = discord_id.to_string();
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT language FROM translation_subscriptions WHERE discord_id = ?",
                )?;
                let result = stmt.query_row([&id], |row| row.get(0)).optional()?;
                Ok(result)
            })
            .await
            .map_err(Into::into)
    }

    /// Get all users subscribed to a language
    pub async fn get_subscribers_for_language(&self, language: &str) -> Result<Vec<String>> {
        let lang = language.to_string();
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT discord_id FROM translation_subscriptions WHERE language = ?",
                )?;
                let rows = stmt
                    .query_map([&lang], |row| row.get(0))?
                    .collect::<Result<Vec<String>, _>>()?;
                Ok(rows)
            })
            .await
            .map_err(Into::into)
    }

    /// Enable translation in a channel
    pub async fn enable_translation_channel(&self, channel_id: &str, enabled_by: &str) -> Result<()> {
        let ch = channel_id.to_string();
        let by = enabled_by.to_string();
        let now = chrono::Utc::now().timestamp();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT OR REPLACE INTO translation_channels
                     (channel_id, enabled_at, enabled_by) VALUES (?, ?, ?)",
                    rusqlite::params![ch, now, by],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    /// Disable translation in a channel
    pub async fn disable_translation_channel(&self, channel_id: &str) -> Result<()> {
        let ch = channel_id.to_string();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "DELETE FROM translation_channels WHERE channel_id = ?",
                    [&ch],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    /// Check if translation is enabled in a channel
    pub async fn is_translation_enabled(&self, channel_id: &str) -> Result<bool> {
        let ch = channel_id.to_string();
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT 1 FROM translation_channels WHERE channel_id = ?",
                )?;
                let exists = stmt.exists([&ch])?;
                Ok(exists)
            })
            .await
            .map_err(Into::into)
    }

    // ==================== Watch Configurations ====================

    /// Set GitHub watch for a channel
    pub async fn set_github_watch(&self, channel_id: &str, repo: &str, level: &str) -> Result<()> {
        let ch = channel_id.to_string();
        let r = repo.to_string();
        let l = level.to_string();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT OR REPLACE INTO github_watches (channel_id, repo, level) VALUES (?, ?, ?)",
                    rusqlite::params![ch, r, l],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    /// Remove GitHub watch from a channel
    pub async fn remove_github_watch(&self, channel_id: &str, repo: &str) -> Result<()> {
        let ch = channel_id.to_string();
        let r = repo.to_string();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "DELETE FROM github_watches WHERE channel_id = ? AND repo = ?",
                    rusqlite::params![ch, r],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    /// Get channels watching a GitHub repo
    pub async fn get_github_watchers(&self, repo: &str) -> Result<Vec<(String, String)>> {
        let r = repo.to_string();
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT channel_id, level FROM github_watches WHERE repo = ?",
                )?;
                let rows = stmt
                    .query_map([&r], |row| Ok((row.get(0)?, row.get(1)?)))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .await
            .map_err(Into::into)
    }

    /// Set Plane watch for a channel
    pub async fn set_plane_watch(&self, channel_id: &str, project: &str, level: &str) -> Result<()> {
        let ch = channel_id.to_string();
        let p = project.to_string();
        let l = level.to_string();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT OR REPLACE INTO plane_watches (channel_id, project, level) VALUES (?, ?, ?)",
                    rusqlite::params![ch, p, l],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    /// Remove Plane watch from a channel
    pub async fn remove_plane_watch(&self, channel_id: &str, project: &str) -> Result<()> {
        let ch = channel_id.to_string();
        let p = project.to_string();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "DELETE FROM plane_watches WHERE channel_id = ? AND project = ?",
                    rusqlite::params![ch, p],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    /// Get channels watching a Plane project
    pub async fn get_plane_watchers(&self, project: &str) -> Result<Vec<(String, String)>> {
        let p = project.to_string();
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT channel_id, level FROM plane_watches WHERE project = ?",
                )?;
                let rows = stmt
                    .query_map([&p], |row| Ok((row.get(0)?, row.get(1)?)))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .await
            .map_err(Into::into)
    }
}

// Re-export Optional from rusqlite for query_row
trait OptionalExt<T> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalExt<T> for Result<T, rusqlite::Error> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
