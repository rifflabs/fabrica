//! Discord bot setup and command registration

use crate::config::Config;
use crate::db::Database;
use crate::modules::{github, plane, status, translation};
use anyhow::Result;
use poise::serenity_prelude::{self as serenity, Mentionable};
use tracing::{error, info};

/// Shared state across all commands
#[derive(Debug)]
pub struct Data {
    pub config: Config,
    pub db: Database,
}

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;

/// Run the Discord bot
pub async fn run(config: Config, db: Database) -> Result<()> {
    let token = config.discord.token.clone();
    let intents = serenity::GatewayIntents::GUILD_MESSAGES
        | serenity::GatewayIntents::DIRECT_MESSAGES
        | serenity::GatewayIntents::MESSAGE_CONTENT
        | serenity::GatewayIntents::GUILDS;

    let data = Data {
        config: config.clone(),
        db: db.clone(),
    };

    // Capture guild_ids before the closure
    let guild_ids = config.discord.guild_ids.clone();

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                // Root command group
                fabrica(),
                // Convenience aliases
                who(),
                team(),
            ],
            event_handler: |ctx, event, framework, data| {
                Box::pin(event_handler(ctx, event, framework, data))
            },
            on_error: |error| {
                Box::pin(async move {
                    error!("Command error: {:?}", error);
                })
            },
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                // Register commands to specified guilds only
                if guild_ids.is_empty() {
                    return Err("No guild_ids configured! Add guild_ids = [\"...\"] to fabrica.toml".into());
                }

                for guild_id_str in &guild_ids {
                    match guild_id_str.parse::<u64>() {
                        Ok(gid) => {
                            poise::builtins::register_in_guild(
                                ctx,
                                &framework.options().commands,
                                serenity::GuildId::new(gid),
                            )
                            .await?;
                            info!("Commands registered to guild {}", gid);
                        }
                        Err(e) => {
                            error!("Invalid guild ID '{}': {}", guild_id_str, e);
                        }
                    }
                }

                info!("Bot restricted to {} guild(s)", guild_ids.len());
                Ok(data)
            })
        })
        .build();

    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await?;

    info!("Palace Fabrica connected to Discord");
    client.start().await?;

    Ok(())
}

/// Event handler for message events (translation) and other Discord events
async fn event_handler(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    _framework: poise::FrameworkContext<'_, Data, Error>,
    data: &Data,
) -> Result<(), Error> {
    match event {
        serenity::FullEvent::Message { new_message } => {
            // Skip bot messages
            if new_message.author.bot {
                return Ok(());
            }

            // Only process messages from allowed guilds
            if let Some(guild_id) = new_message.guild_id {
                let guild_id_str = guild_id.to_string();
                if !data.config.discord.guild_ids.contains(&guild_id_str) {
                    return Ok(());
                }
            } else {
                // Skip DMs
                return Ok(());
            }

            // Handle translation
            translation::handle_message(ctx, new_message, data).await?;
        }
        serenity::FullEvent::Ready { data_about_bot } => {
            info!("Bot ready as {}", data_about_bot.user.name);
        }
        _ => {}
    }
    Ok(())
}

// ==================== Root Command ====================

/// Palace Fabrica - Coordination infrastructure
#[poise::command(
    slash_command,
    prefix_command,
    subcommands(
        "translate_cmd",
        "server_cmd",
        "last_fabrica",
        "available_cmd",
        "busy_cmd",
        "away_cmd",
        "clear_cmd",
        "hours_cmd",
        "settings_cmd",
        "who_cmd",
        "team_cmd",
        "project_cmd",
        "issues_cmd",
        "sprint_cmd",
        "repo_cmd",
        "commits_cmd",
        "prs_cmd",
        "watch_cmd",
        "unwatch_cmd",
    )
)]
pub async fn fabrica(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("Palace Fabrica - Use `/fabrica help` for available commands").await?;
    Ok(())
}

// ==================== Translation Commands ====================

/// Translation commands
#[poise::command(slash_command, prefix_command, subcommands("subscribe", "unsubscribe", "status_sub", "mode_set", "mode_show", "debug_mode", "last_cmd"), rename = "translate")]
pub async fn translate_cmd(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Server management commands
#[poise::command(slash_command, prefix_command, subcommands("server_status", "server_permissions", "server_allow", "server_deny"), rename = "server")]
pub async fn server_cmd(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Show recent messages translated to your subscribed language (shortcut for /fabrica translate last)
#[poise::command(slash_command, prefix_command, rename = "last")]
pub async fn last_fabrica(
    ctx: Context<'_>,
    #[description = "Number of messages to show (max 100)"] count: Option<u32>,
) -> Result<(), Error> {
    translation::last(ctx, count).await
}

/// Show server translation status
#[poise::command(slash_command, prefix_command, rename = "status")]
pub async fn server_status(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = match ctx.guild_id() {
        Some(gid) => gid.to_string(),
        None => {
            ctx.say("⚠️ This command is only available in servers.").await?;
            return Ok(());
        }
    };

    // Get all permissions for this guild
    let permissions = ctx.data().db.get_guild_permissions(&guild_id).await?;

    if permissions.is_empty() {
        ctx.say("📊 **Server Status**\n\n\
                 No custom role permissions configured.\n\
                 Only users with **MANAGE_CHANNELS** or **ADMINISTRATOR** can manage translation settings.\n\n\
                 Use `/fabrica server allow mode @role` to grant a role permission to change translation modes.")
            .await?;
    } else {
        let mut mode_targets = Vec::new();
        let mut admin_targets = Vec::new();

        for (role_id, permission) in permissions {
            let target_display = if role_id == "everyone" {
                "**everyone**".to_string()
            } else {
                format!("<@&{}>", role_id)
            };
            match permission.as_str() {
                "mode" => mode_targets.push(target_display),
                "admin" => admin_targets.push(target_display),
                _ => {}
            }
        }

        let mut msg = String::from("📊 **Server Status**\n\n");

        if !admin_targets.is_empty() {
            msg.push_str(&format!("**Admin:** {}\n", admin_targets.join(", ")));
        }
        if !mode_targets.is_empty() {
            msg.push_str(&format!("**Mode:** {}\n", mode_targets.join(", ")));
        }

        msg.push_str("\n_Admin can manage all settings. Mode can change translation modes._");

        ctx.say(msg).await?;
    }

    Ok(())
}

/// Show configured permissions for this server
#[poise::command(slash_command, prefix_command, rename = "permissions")]
pub async fn server_permissions(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = match ctx.guild_id() {
        Some(gid) => gid.to_string(),
        None => {
            ctx.say("⚠️ This command is only available in servers.").await?;
            return Ok(());
        }
    };

    // Get all permissions for this guild
    let permissions = ctx.data().db.get_guild_permissions(&guild_id).await?;

    if permissions.is_empty() {
        ctx.say("📊 **Server Permissions**\n\n\
                 No custom role permissions configured.\n\
                 Only users with **MANAGE_CHANNELS** or **ADMINISTRATOR** can manage translation settings.\n\n\
                 Use `/fabrica server allow mode @role` to grant a role permission to change translation modes.")
            .await?;
    } else {
        let mut mode_targets = Vec::new();
        let mut admin_targets = Vec::new();

        for (role_id, permission) in permissions {
            let target_display = if role_id == "everyone" {
                "**everyone**".to_string()
            } else {
                format!("<@&{}>", role_id)
            };
            match permission.as_str() {
                "mode" => mode_targets.push(target_display),
                "admin" => admin_targets.push(target_display),
                _ => {}
            }
        }

        let mut msg = String::from("📊 **Server Permissions**\n\n");

        if !admin_targets.is_empty() {
            msg.push_str(&format!("**Admin:** {}\n", admin_targets.join(", ")));
        }
        if !mode_targets.is_empty() {
            msg.push_str(&format!("**Mode:** {}\n", mode_targets.join(", ")));
        }

        msg.push_str("\n_Admin can manage all settings. Mode can change translation modes._");

        ctx.say(msg).await?;
    }

    Ok(())
}

/// Allow a role or everyone to manage translation settings
#[poise::command(slash_command, prefix_command, rename = "allow")]
pub async fn server_allow(
    ctx: Context<'_>,
    #[description = "Permission type: mode or admin"] permission: String,
    #[description = "Role to grant permission (or 'everyone')"] role: Option<serenity::Role>,
    #[description = "Grant to everyone (type 'everyone')"] everyone: Option<String>,
) -> Result<(), Error> {
    let guild_id = match ctx.guild_id() {
        Some(gid) => gid.to_string(),
        None => {
            ctx.say("⚠️ This command is only available in servers.").await?;
            return Ok(());
        }
    };

    // Check if user has admin permission
    if !translation::has_admin_permission(&ctx, &guild_id).await {
        ctx.say("⚠️ You need **ADMINISTRATOR** permission to manage server settings.").await?;
        return Ok(());
    }

    let permission_lower = permission.to_lowercase();
    if !matches!(permission_lower.as_str(), "mode" | "admin") {
        ctx.say("⚠️ Invalid permission. Available permissions:\n\
                 • **mode** - Can change translation modes\n\
                 • **admin** - Can manage all Fabrica settings").await?;
        return Ok(());
    }

    // Determine target: "everyone" or a specific role
    let (target_id, target_display) = if let Some(ref everyone_str) = everyone {
        if everyone_str.to_lowercase() == "everyone" {
            ("everyone".to_string(), "**everyone**".to_string())
        } else {
            ctx.say("⚠️ Invalid target. Use `everyone` or select a role.").await?;
            return Ok(());
        }
    } else if let Some(ref role) = role {
        (role.id.to_string(), role.mention().to_string())
    } else {
        ctx.say("⚠️ Please specify a role or use `everyone` to grant permission to all users.\n\
                 Example: `/fabrica server allow mode @role` or `/fabrica server allow mode everyone:everyone`").await?;
        return Ok(());
    };

    let granted_by = ctx.author().id.to_string();

    ctx.data().db.add_guild_permission(&guild_id, &target_id, &permission_lower, &granted_by).await?;

    let permission_desc = match permission_lower.as_str() {
        "mode" => "change translation modes",
        "admin" => "manage all Fabrica settings",
        _ => "unknown",
    };

    ctx.say(format!("✅ {} can now {}.", target_display, permission_desc)).await?;

    Ok(())
}

/// Revoke a role's or everyone's permission to manage translation settings
#[poise::command(slash_command, prefix_command, rename = "deny")]
pub async fn server_deny(
    ctx: Context<'_>,
    #[description = "Permission type: mode or admin"] permission: String,
    #[description = "Role to revoke permission from"] role: Option<serenity::Role>,
    #[description = "Revoke from everyone (type 'everyone')"] everyone: Option<String>,
) -> Result<(), Error> {
    let guild_id = match ctx.guild_id() {
        Some(gid) => gid.to_string(),
        None => {
            ctx.say("⚠️ This command is only available in servers.").await?;
            return Ok(());
        }
    };

    // Check if user has admin permission
    if !translation::has_admin_permission(&ctx, &guild_id).await {
        ctx.say("⚠️ You need **ADMINISTRATOR** permission to manage server settings.").await?;
        return Ok(());
    }

    let permission_lower = permission.to_lowercase();
    if !matches!(permission_lower.as_str(), "mode" | "admin") {
        ctx.say("⚠️ Invalid permission. Available permissions:\n\
                 • **mode** - Can change translation modes\n\
                 • **admin** - Can manage all Fabrica settings").await?;
        return Ok(());
    }

    // Determine target: "everyone" or a specific role
    let (target_id, target_display) = if let Some(ref everyone_str) = everyone {
        if everyone_str.to_lowercase() == "everyone" {
            ("everyone".to_string(), "**everyone**".to_string())
        } else {
            ctx.say("⚠️ Invalid target. Use `everyone` or select a role.").await?;
            return Ok(());
        }
    } else if let Some(ref role) = role {
        (role.id.to_string(), role.mention().to_string())
    } else {
        ctx.say("⚠️ Please specify a role or use `everyone` to revoke permission.\n\
                 Example: `/fabrica server deny mode @role` or `/fabrica server deny mode everyone:everyone`").await?;
        return Ok(());
    };

    ctx.data().db.remove_guild_permission(&guild_id, &target_id, &permission_lower).await?;

    let permission_desc = match permission_lower.as_str() {
        "mode" => "change translation modes",
        "admin" => "manage all Fabrica settings",
        _ => "unknown",
    };

    ctx.say(format!("✅ {} can no longer {}.", target_display, permission_desc)).await?;

    Ok(())
}

/// Subscribe to receive translations in your preferred language (en, hi, fr)
#[poise::command(slash_command, prefix_command)]
pub async fn subscribe(
    ctx: Context<'_>,
    #[description = "Language to receive translations in (en, hi, fr)"] language: String,
) -> Result<(), Error> {
    translation::subscribe(ctx, language).await
}

/// Stop receiving translation DMs (specify language or 'all')
#[poise::command(slash_command, prefix_command)]
pub async fn unsubscribe(
    ctx: Context<'_>,
    #[description = "Language to unsubscribe from (or 'all')"] language: Option<String>,
) -> Result<(), Error> {
    translation::unsubscribe(ctx, language).await
}

/// Show your translation settings
#[poise::command(slash_command, prefix_command, rename = "status")]
pub async fn status_sub(ctx: Context<'_>) -> Result<(), Error> {
    translation::status(ctx).await
}

/// Set translation mode for this channel (off/silent/on/transparent)
#[poise::command(slash_command, prefix_command, rename = "mode")]
pub async fn mode_set(
    ctx: Context<'_>,
    #[description = "Translation mode: off, silent, on, or transparent"] mode: String,
) -> Result<(), Error> {
    translation::set_mode(ctx, mode).await
}

/// Show current translation mode for this channel
#[poise::command(slash_command, prefix_command, rename = "info")]
pub async fn mode_show(ctx: Context<'_>) -> Result<(), Error> {
    translation::show_mode(ctx).await
}

/// Toggle debug mode (receive translations of your own messages)
#[poise::command(slash_command, prefix_command, rename = "debug")]
pub async fn debug_mode(ctx: Context<'_>) -> Result<(), Error> {
    translation::debug(ctx).await
}

/// Show recent messages translated to your subscribed language
#[poise::command(slash_command, prefix_command, rename = "last")]
pub async fn last_cmd(
    ctx: Context<'_>,
    #[description = "Number of messages to show (max 100)"] count: Option<u32>,
) -> Result<(), Error> {
    translation::last(ctx, count).await
}

// ==================== Status Commands ====================

/// Mark yourself as available
#[poise::command(slash_command, prefix_command, rename = "available")]
pub async fn available_cmd(
    ctx: Context<'_>,
    #[description = "What you're working on (optional)"] message: Option<String>,
) -> Result<(), Error> {
    status::set_available(ctx, message).await
}

/// Mark yourself as busy
#[poise::command(slash_command, prefix_command, rename = "busy")]
pub async fn busy_cmd(
    ctx: Context<'_>,
    #[description = "What you're focused on (optional)"] message: Option<String>,
) -> Result<(), Error> {
    status::set_busy(ctx, message).await
}

/// Mark yourself as away
#[poise::command(slash_command, prefix_command, rename = "away")]
pub async fn away_cmd(
    ctx: Context<'_>,
    #[description = "When you'll be back (optional)"] message: Option<String>,
) -> Result<(), Error> {
    status::set_away(ctx, message).await
}

/// Clear your status
#[poise::command(slash_command, prefix_command, rename = "clear")]
pub async fn clear_cmd(ctx: Context<'_>) -> Result<(), Error> {
    status::clear(ctx).await
}

/// Set your working hours
#[poise::command(slash_command, prefix_command, rename = "hours")]
pub async fn hours_cmd(
    ctx: Context<'_>,
    #[description = "Schedule (e.g., 'M-F 9:30 to 17:30' or 'until 23:30')"]
    #[rest]
    schedule: Option<String>,
) -> Result<(), Error> {
    match schedule {
        Some(s) if !s.trim().is_empty() => status::set_hours(ctx, s).await,
        _ => status::show_hours(ctx).await,
    }
}

/// User settings
#[poise::command(
    slash_command,
    prefix_command,
    rename = "settings",
    subcommands("settings_timezone", "settings_format"),
)]
pub async fn settings_cmd(ctx: Context<'_>) -> Result<(), Error> {
    status::show_settings(ctx).await
}

/// Set your timezone (admins can set for others)
#[poise::command(slash_command, prefix_command, rename = "timezone")]
pub async fn settings_timezone(
    ctx: Context<'_>,
    #[description = "Timezone (e.g., 'London', 'New York', 'Europe/Paris')"]
    timezone: String,
    #[description = "User to set timezone for (admin only)"]
    user: Option<serenity::User>,
) -> Result<(), Error> {
    status::set_timezone(ctx, timezone, user).await
}

/// Set your time format
#[poise::command(slash_command, prefix_command, rename = "format")]
pub async fn settings_format(
    ctx: Context<'_>,
    #[description = "Time format: 24h or 12h"]
    format: String,
) -> Result<(), Error> {
    status::set_time_format(ctx, format).await
}

/// Show who's currently available
#[poise::command(slash_command, prefix_command, rename = "who")]
pub async fn who_cmd(ctx: Context<'_>) -> Result<(), Error> {
    status::who(ctx).await
}

/// Show available team members (use 'public' to make visible to everyone)
#[poise::command(slash_command, prefix_command, rename = "team")]
pub async fn team_cmd(
    ctx: Context<'_>,
    #[description = "Make visible to everyone (type 'public')"]
    visibility: Option<String>,
) -> Result<(), Error> {
    let public = visibility.map(|v| v.trim().eq_ignore_ascii_case("public") || v.trim() == "!").unwrap_or(false);
    status::team(ctx, public).await
}

// ==================== Plane Commands ====================

/// Show project overview
#[poise::command(slash_command, prefix_command, rename = "project")]
pub async fn project_cmd(
    ctx: Context<'_>,
    #[description = "Project name"] name: String,
) -> Result<(), Error> {
    plane::project(ctx, name).await
}

/// List issues
#[poise::command(slash_command, prefix_command, rename = "issues")]
pub async fn issues_cmd(
    ctx: Context<'_>,
    #[description = "Project name (optional)"] project: Option<String>,
    #[description = "Filter by status (optional)"] status: Option<String>,
) -> Result<(), Error> {
    plane::issues(ctx, project, status).await
}

/// Show current sprint status
#[poise::command(slash_command, prefix_command, rename = "sprint")]
pub async fn sprint_cmd(
    ctx: Context<'_>,
    #[description = "Project name (optional)"] project: Option<String>,
) -> Result<(), Error> {
    plane::sprint(ctx, project).await
}

// ==================== GitHub Commands ====================

/// Show repository status
#[poise::command(slash_command, prefix_command, rename = "repo")]
pub async fn repo_cmd(
    ctx: Context<'_>,
    #[description = "Repository name"] name: String,
) -> Result<(), Error> {
    github::repo(ctx, name).await
}

/// Show recent commits
#[poise::command(slash_command, prefix_command, rename = "commits")]
pub async fn commits_cmd(
    ctx: Context<'_>,
    #[description = "Repository name"] repo: String,
    #[description = "Number of commits to show"] count: Option<u32>,
) -> Result<(), Error> {
    github::commits(ctx, repo, count).await
}

/// Show open pull requests
#[poise::command(slash_command, prefix_command, rename = "prs")]
pub async fn prs_cmd(
    ctx: Context<'_>,
    #[description = "Repository name"] repo: String,
) -> Result<(), Error> {
    github::prs(ctx, repo).await
}

// ==================== Watch Commands ====================

/// Watch a repository or project in this channel
#[poise::command(slash_command, prefix_command, required_permissions = "MANAGE_CHANNELS")]
pub async fn watch_cmd(
    ctx: Context<'_>,
    #[description = "Type: 'github' or 'plane'"] watch_type: String,
    #[description = "Repository or project name"] name: String,
    #[description = "Level: all, important, minimal"] level: Option<String>,
) -> Result<(), Error> {
    let level = level.unwrap_or_else(|| "important".to_string());
    match watch_type.to_lowercase().as_str() {
        "github" => github::watch(ctx, name, level).await,
        "plane" => plane::watch(ctx, name, level).await,
        _ => {
            ctx.say("Unknown watch type. Use 'github' or 'plane'.").await?;
            Ok(())
        }
    }
}

/// Stop watching a repository or project
#[poise::command(slash_command, prefix_command, required_permissions = "MANAGE_CHANNELS")]
pub async fn unwatch_cmd(
    ctx: Context<'_>,
    #[description = "Type: 'github' or 'plane'"] watch_type: String,
    #[description = "Repository or project name"] name: String,
) -> Result<(), Error> {
    match watch_type.to_lowercase().as_str() {
        "github" => github::unwatch(ctx, name).await,
        "plane" => plane::unwatch(ctx, name).await,
        _ => {
            ctx.say("Unknown watch type. Use 'github' or 'plane'.").await?;
            Ok(())
        }
    }
}

// ==================== Top-Level Aliases ====================

/// Show who's currently available (alias for /fabrica who)
#[poise::command(slash_command, prefix_command)]
pub async fn who(ctx: Context<'_>) -> Result<(), Error> {
    status::who(ctx).await
}

/// Show full team status (alias for /fabrica team)
#[poise::command(slash_command, prefix_command)]
pub async fn team(
    ctx: Context<'_>,
    #[description = "Make visible to everyone (type 'public')"]
    visibility: Option<String>,
) -> Result<(), Error> {
    let public = visibility.map(|v| v.trim().eq_ignore_ascii_case("public") || v.trim() == "!").unwrap_or(false);
    status::team(ctx, public).await
}
