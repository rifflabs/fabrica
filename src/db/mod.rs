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

                // Run incremental migrations (ignore errors for already-applied migrations)
                let _ = conn.execute_batch(schema::MIGRATION_ADD_DEBUG_MODE);
                let _ = conn.execute_batch(schema::MIGRATION_ADD_CHANNEL_MODE);
                let _ = conn.execute_batch(schema::MIGRATION_ADD_CHANNEL_TO_SUBS);
                let _ = conn.execute_batch(schema::MIGRATION_ADD_GUILD_TO_SUBS);
                let _ = conn.execute_batch(schema::MIGRATION_ADD_GUILD_TO_CHANNELS);

                // Create indexes after all columns exist
                let _ = conn.execute_batch(schema::MIGRATION_CREATE_GUILD_INDEXES);

                // Fix primary keys for tables that were altered
                let _ = conn.execute_batch(schema::MIGRATION_FIX_TRANSLATION_CHANNELS_PK);
                let _ = conn.execute_batch(schema::MIGRATION_FIX_TRANSLATION_SUBS_PK);

                // Add user schedule tables
                let _ = conn.execute_batch(schema::MIGRATION_ADD_USER_SCHEDULES);

                // Fix user schedule tables to include guild_id
                let _ = conn.execute_batch(schema::MIGRATION_FIX_USER_WEEKLY_SCHEDULE_PK);
                let _ = conn.execute_batch(schema::MIGRATION_FIX_USER_SCHEDULE_OVERRIDE_PK);

                // Add user settings table
                let _ = conn.execute_batch(schema::MIGRATION_ADD_USER_SETTINGS);

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

    // ==================== User Schedule ====================

    /// Set weekly schedule for specific days (per guild)
    pub async fn set_weekly_schedule(&self, guild_id: &str, discord_id: &str, days: &[u8], start_time: &str, end_time: &str) -> Result<()> {
        let gid = guild_id.to_string();
        let id = discord_id.to_string();
        let start = start_time.to_string();
        let end = end_time.to_string();
        let days_vec: Vec<u8> = days.to_vec();

        self.conn
            .call(move |conn| {
                for day in days_vec {
                    conn.execute(
                        "INSERT OR REPLACE INTO user_weekly_schedule (guild_id, discord_id, day_of_week, start_time, end_time)
                         VALUES (?, ?, ?, ?, ?)",
                        rusqlite::params![gid, id, day, start, end],
                    )?;
                }
                Ok(())
            })
            .await?;
        Ok(())
    }

    /// Get weekly schedule for a user in a guild
    pub async fn get_weekly_schedule(&self, guild_id: &str, discord_id: &str) -> Result<Vec<(u8, String, String)>> {
        let gid = guild_id.to_string();
        let id = discord_id.to_string();
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT day_of_week, start_time, end_time FROM user_weekly_schedule
                     WHERE guild_id = ? AND discord_id = ? ORDER BY day_of_week"
                )?;
                let rows = stmt
                    .query_map([&gid, &id], |row| {
                        Ok((row.get::<_, u8>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .await
            .map_err(Into::into)
    }

    /// Set a one-off schedule override for a specific date (per guild)
    pub async fn set_schedule_override(&self, guild_id: &str, discord_id: &str, date: &str, start_time: Option<&str>, end_time: &str) -> Result<()> {
        let gid = guild_id.to_string();
        let id = discord_id.to_string();
        let d = date.to_string();
        let start = start_time.map(|s| s.to_string());
        let end = end_time.to_string();

        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT OR REPLACE INTO user_schedule_override (guild_id, discord_id, date, start_time, end_time)
                     VALUES (?, ?, ?, ?, ?)",
                    rusqlite::params![gid, id, d, start, end],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    /// Get schedule override for a specific date in a guild
    pub async fn get_schedule_override(&self, guild_id: &str, discord_id: &str, date: &str) -> Result<Option<(Option<String>, String)>> {
        let gid = guild_id.to_string();
        let id = discord_id.to_string();
        let d = date.to_string();
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT start_time, end_time FROM user_schedule_override
                     WHERE guild_id = ? AND discord_id = ? AND date = ?"
                )?;
                let result = stmt
                    .query_row([&gid, &id, &d], |row| {
                        Ok((row.get::<_, Option<String>>(0)?, row.get::<_, String>(1)?))
                    })
                    .optional()?;
                Ok(result)
            })
            .await
            .map_err(Into::into)
    }

    /// Clear old schedule overrides (before a given date)
    pub async fn clear_old_schedule_overrides(&self, before_date: &str) -> Result<()> {
        let d = before_date.to_string();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "DELETE FROM user_schedule_override WHERE date < ?",
                    [&d],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    // ==================== User Settings ====================

    /// Get user settings (returns defaults if not set)
    pub async fn get_user_settings(&self, discord_id: &str) -> Result<UserSettings> {
        let id = discord_id.to_string();
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT discord_id, timezone, time_format FROM user_settings WHERE discord_id = ?",
                )?;
                let result = stmt
                    .query_row([&id], |row| {
                        Ok(UserSettings {
                            discord_id: row.get(0)?,
                            timezone: row.get(1)?,
                            time_format: row.get(2)?,
                        })
                    })
                    .optional()?;
                Ok(result.unwrap_or_else(|| UserSettings::new(&id)))
            })
            .await
            .map_err(Into::into)
    }

    /// Set user timezone
    pub async fn set_user_timezone(&self, discord_id: &str, timezone: &str) -> Result<()> {
        let id = discord_id.to_string();
        let tz = timezone.to_string();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO user_settings (discord_id, timezone, time_format)
                     VALUES (?, ?, '24h')
                     ON CONFLICT(discord_id) DO UPDATE SET timezone = excluded.timezone",
                    rusqlite::params![id, tz],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    /// Set user time format
    pub async fn set_user_time_format(&self, discord_id: &str, time_format: &str) -> Result<()> {
        let id = discord_id.to_string();
        let fmt = time_format.to_string();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO user_settings (discord_id, timezone, time_format)
                     VALUES (?, 'UTC', ?)
                     ON CONFLICT(discord_id) DO UPDATE SET time_format = excluded.time_format",
                    rusqlite::params![id, fmt],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    // ==================== Translation ====================

    /// Add a translation subscription for a channel in a guild
    pub async fn add_translation_subscription(&self, guild_id: &str, discord_id: &str, channel_id: &str, language: &str) -> Result<()> {
        let gid = guild_id.to_string();
        let id = discord_id.to_string();
        let ch = channel_id.to_string();
        let lang = language.to_string();
        let now = chrono::Utc::now().timestamp();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT OR REPLACE INTO translation_subscriptions
                     (guild_id, discord_id, channel_id, language, created_at, debug_mode) VALUES (?, ?, ?, ?, ?, 0)",
                    rusqlite::params![gid, id, ch, lang, now],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    /// Remove a specific language subscription for a channel in a guild
    pub async fn remove_translation_subscription(&self, guild_id: &str, discord_id: &str, channel_id: &str, language: &str) -> Result<()> {
        let gid = guild_id.to_string();
        let id = discord_id.to_string();
        let ch = channel_id.to_string();
        let lang = language.to_string();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "DELETE FROM translation_subscriptions
                     WHERE guild_id = ? AND discord_id = ? AND channel_id = ? AND language = ?",
                    rusqlite::params![gid, id, ch, lang],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    /// Remove all translation subscriptions for a channel in a guild
    pub async fn remove_all_translation_subscriptions(&self, guild_id: &str, discord_id: &str, channel_id: &str) -> Result<()> {
        let gid = guild_id.to_string();
        let id = discord_id.to_string();
        let ch = channel_id.to_string();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "DELETE FROM translation_subscriptions
                     WHERE guild_id = ? AND discord_id = ? AND channel_id = ?",
                    rusqlite::params![gid, id, ch],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    /// Get all of a user's translation subscriptions for a channel in a guild
    pub async fn get_translation_subscriptions(&self, guild_id: &str, discord_id: &str, channel_id: &str) -> Result<Vec<String>> {
        let gid = guild_id.to_string();
        let id = discord_id.to_string();
        let ch = channel_id.to_string();
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT language FROM translation_subscriptions
                     WHERE guild_id = ? AND discord_id = ? AND channel_id = ?",
                )?;
                let rows = stmt
                    .query_map(rusqlite::params![gid, id, ch], |row| row.get(0))?
                    .collect::<Result<Vec<String>, _>>()?;
                Ok(rows)
            })
            .await
            .map_err(Into::into)
    }

    /// Check if a user is subscribed to a specific language in a channel in a guild
    pub async fn has_translation_subscription(&self, guild_id: &str, discord_id: &str, channel_id: &str, language: &str) -> Result<bool> {
        let gid = guild_id.to_string();
        let id = discord_id.to_string();
        let ch = channel_id.to_string();
        let lang = language.to_string();
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT 1 FROM translation_subscriptions
                     WHERE guild_id = ? AND discord_id = ? AND channel_id = ? AND language = ?",
                )?;
                let exists = stmt.query_row(rusqlite::params![gid, id, ch, lang], |_| Ok(true)).optional()?.unwrap_or(false);
                Ok(exists)
            })
            .await
            .map_err(Into::into)
    }

    /// Get all users subscribed to a language in a channel in a guild
    pub async fn get_channel_subscribers_for_language(&self, guild_id: &str, channel_id: &str, language: &str) -> Result<Vec<String>> {
        let gid = guild_id.to_string();
        let ch = channel_id.to_string();
        let lang = language.to_string();
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT DISTINCT discord_id FROM translation_subscriptions
                     WHERE guild_id = ? AND channel_id = ? AND language = ?",
                )?;
                let rows = stmt
                    .query_map(rusqlite::params![gid, ch, lang], |row| row.get(0))?
                    .collect::<Result<Vec<String>, _>>()?;
                Ok(rows)
            })
            .await
            .map_err(Into::into)
    }

    /// Get all non-English subscriptions for a channel in a guild (discord_id, language)
    pub async fn get_channel_non_english_subscriptions(&self, guild_id: &str, channel_id: &str) -> Result<Vec<(String, String)>> {
        let gid = guild_id.to_string();
        let ch = channel_id.to_string();
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT DISTINCT discord_id, language FROM translation_subscriptions
                     WHERE guild_id = ? AND channel_id = ? AND language != 'en'",
                )?;
                let rows = stmt
                    .query_map(rusqlite::params![gid, ch], |row| Ok((row.get(0)?, row.get(1)?)))?
                    .collect::<Result<Vec<(String, String)>, _>>()?;
                Ok(rows)
            })
            .await
            .map_err(Into::into)
    }

    /// Get all unique languages subscribed to in a channel in a guild (for transparent mode)
    pub async fn get_channel_subscribed_languages(&self, guild_id: &str, channel_id: &str) -> Result<Vec<String>> {
        let gid = guild_id.to_string();
        let ch = channel_id.to_string();
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT DISTINCT language FROM translation_subscriptions
                     WHERE guild_id = ? AND channel_id = ?",
                )?;
                let rows = stmt
                    .query_map(rusqlite::params![gid, ch], |row| row.get(0))?
                    .collect::<Result<Vec<String>, _>>()?;
                Ok(rows)
            })
            .await
            .map_err(Into::into)
    }

    /// Toggle translation debug mode for a user in a channel in a guild
    pub async fn set_translation_debug_mode(&self, guild_id: &str, discord_id: &str, channel_id: &str, enabled: bool) -> Result<()> {
        let gid = guild_id.to_string();
        let id = discord_id.to_string();
        let ch = channel_id.to_string();
        let debug_val = if enabled { 1 } else { 0 };
        self.conn
            .call(move |conn| {
                conn.execute(
                    "UPDATE translation_subscriptions SET debug_mode = ? WHERE guild_id = ? AND discord_id = ? AND channel_id = ?",
                    rusqlite::params![debug_val, gid, id, ch],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    /// Check if a user has debug mode enabled for a channel in a guild
    pub async fn get_translation_debug_mode(&self, guild_id: &str, discord_id: &str, channel_id: &str) -> Result<bool> {
        let gid = guild_id.to_string();
        let id = discord_id.to_string();
        let ch = channel_id.to_string();
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT debug_mode FROM translation_subscriptions WHERE guild_id = ? AND discord_id = ? AND channel_id = ?",
                )?;
                let result: Option<i32> = stmt.query_row(rusqlite::params![gid, id, ch], |row| row.get(0)).optional()?;
                Ok(result.unwrap_or(0) == 1)
            })
            .await
            .map_err(Into::into)
    }

    /// Set translation mode for a channel in a guild
    pub async fn set_channel_translation_mode(&self, guild_id: &str, channel_id: &str, mode: &str, set_by: &str) -> Result<()> {
        let gid = guild_id.to_string();
        let ch = channel_id.to_string();
        let m = mode.to_string();
        let by = set_by.to_string();
        let now = chrono::Utc::now().timestamp();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO translation_channels (guild_id, channel_id, mode, enabled_at, enabled_by)
                     VALUES (?, ?, ?, ?, ?)
                     ON CONFLICT(guild_id, channel_id) DO UPDATE SET mode = excluded.mode, enabled_at = excluded.enabled_at, enabled_by = excluded.enabled_by",
                    rusqlite::params![gid, ch, m, now, by],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    /// Get translation mode for a channel in a guild (returns "off" if not set)
    pub async fn get_channel_translation_mode(&self, guild_id: &str, channel_id: &str) -> Result<String> {
        let gid = guild_id.to_string();
        let ch = channel_id.to_string();
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT mode FROM translation_channels WHERE guild_id = ? AND channel_id = ?",
                )?;
                let result: Option<String> = stmt.query_row(rusqlite::params![gid, ch], |row| row.get(0)).optional()?;
                Ok(result.unwrap_or_else(|| "off".to_string()))
            })
            .await
            .map_err(Into::into)
    }

    /// Check if translation is enabled in a channel in a guild (any mode except "off")
    pub async fn is_translation_enabled(&self, guild_id: &str, channel_id: &str) -> Result<bool> {
        let mode = self.get_channel_translation_mode(guild_id, channel_id).await?;
        Ok(mode != "off")
    }

    // ==================== Guild Permissions ====================

    /// Add a permission for a role in a guild
    pub async fn add_guild_permission(&self, guild_id: &str, role_id: &str, permission: &str, granted_by: &str) -> Result<()> {
        let gid = guild_id.to_string();
        let rid = role_id.to_string();
        let perm = permission.to_string();
        let by = granted_by.to_string();
        let now = chrono::Utc::now().timestamp();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT OR REPLACE INTO guild_permissions (guild_id, role_id, permission, granted_at, granted_by)
                     VALUES (?, ?, ?, ?, ?)",
                    rusqlite::params![gid, rid, perm, now, by],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    /// Remove a permission for a role in a guild
    pub async fn remove_guild_permission(&self, guild_id: &str, role_id: &str, permission: &str) -> Result<()> {
        let gid = guild_id.to_string();
        let rid = role_id.to_string();
        let perm = permission.to_string();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "DELETE FROM guild_permissions WHERE guild_id = ? AND role_id = ? AND permission = ?",
                    rusqlite::params![gid, rid, perm],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    /// Get all permissions for a guild (returns Vec<(role_id, permission)>)
    pub async fn get_guild_permissions(&self, guild_id: &str) -> Result<Vec<(String, String)>> {
        let gid = guild_id.to_string();
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT role_id, permission FROM guild_permissions WHERE guild_id = ?",
                )?;
                let rows = stmt
                    .query_map([&gid], |row| Ok((row.get(0)?, row.get(1)?)))?
                    .collect::<Result<Vec<(String, String)>, _>>()?;
                Ok(rows)
            })
            .await
            .map_err(Into::into)
    }

    /// Get all role IDs with a specific permission in a guild
    pub async fn get_roles_with_permission(&self, guild_id: &str, permission: &str) -> Result<Vec<String>> {
        let gid = guild_id.to_string();
        let perm = permission.to_string();
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT role_id FROM guild_permissions WHERE guild_id = ? AND permission = ?",
                )?;
                let rows = stmt
                    .query_map(rusqlite::params![gid, perm], |row| row.get(0))?
                    .collect::<Result<Vec<String>, _>>()?;
                Ok(rows)
            })
            .await
            .map_err(Into::into)
    }

    // ==================== Last Command Usage ====================

    /// Get the last time a user used /fabrica last in a channel (returns timestamp and optional message_id)
    pub async fn get_last_command_usage(&self, guild_id: &str, channel_id: &str, discord_id: &str) -> Result<Option<(i64, Option<String>)>> {
        let gid = guild_id.to_string();
        let ch = channel_id.to_string();
        let id = discord_id.to_string();
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT last_used_at, last_message_id FROM last_command_usage
                     WHERE guild_id = ? AND channel_id = ? AND discord_id = ?",
                )?;
                let result = stmt
                    .query_row(rusqlite::params![gid, ch, id], |row| {
                        Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
                    })
                    .optional()?;
                Ok(result)
            })
            .await
            .map_err(Into::into)
    }

    /// Set the last time a user used /fabrica last in a channel
    pub async fn set_last_command_usage(&self, guild_id: &str, channel_id: &str, discord_id: &str, message_id: Option<&str>) -> Result<()> {
        let gid = guild_id.to_string();
        let ch = channel_id.to_string();
        let id = discord_id.to_string();
        let msg_id = message_id.map(|s| s.to_string());
        let now = chrono::Utc::now().timestamp();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO last_command_usage (guild_id, channel_id, discord_id, last_used_at, last_message_id)
                     VALUES (?, ?, ?, ?, ?)
                     ON CONFLICT(guild_id, channel_id, discord_id) DO UPDATE SET
                        last_used_at = excluded.last_used_at,
                        last_message_id = excluded.last_message_id",
                    rusqlite::params![gid, ch, id, now, msg_id],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
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
