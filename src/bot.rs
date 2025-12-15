//! Discord bot setup and command registration

use crate::config::Config;
use crate::db::Database;
use crate::modules::{github, plane, status, translation};
use anyhow::Result;
use poise::serenity_prelude as serenity;
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

    // Capture guild_id before the closure
    let guild_id = config.discord.guild_id;

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
                // Register commands globally or to a specific guild
                if let Some(gid) = guild_id {
                    poise::builtins::register_in_guild(
                        ctx,
                        &framework.options().commands,
                        serenity::GuildId::new(gid),
                    )
                    .await?;
                    info!("Commands registered to guild {}", gid);
                } else {
                    poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                    info!("Commands registered globally");
                }
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
        "available_cmd",
        "busy_cmd",
        "away_cmd",
        "clear_cmd",
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
#[poise::command(slash_command, prefix_command, subcommands("subscribe", "unsubscribe", "status_sub", "enable", "disable"), rename = "translate")]
pub async fn translate_cmd(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Subscribe to receive translations in your preferred language
#[poise::command(slash_command, prefix_command)]
pub async fn subscribe(
    ctx: Context<'_>,
    #[description = "Language to receive translations in (e.g., 'hindi', 'hi')"] language: String,
) -> Result<(), Error> {
    translation::subscribe(ctx, language).await
}

/// Stop receiving translation DMs
#[poise::command(slash_command, prefix_command)]
pub async fn unsubscribe(ctx: Context<'_>) -> Result<(), Error> {
    translation::unsubscribe(ctx).await
}

/// Show your translation settings
#[poise::command(slash_command, prefix_command, rename = "status")]
pub async fn status_sub(ctx: Context<'_>) -> Result<(), Error> {
    translation::status(ctx).await
}

/// Enable translation in this channel (admin only)
#[poise::command(slash_command, prefix_command, required_permissions = "MANAGE_CHANNELS")]
pub async fn enable(ctx: Context<'_>) -> Result<(), Error> {
    translation::enable_channel(ctx).await
}

/// Disable translation in this channel (admin only)
#[poise::command(slash_command, prefix_command, required_permissions = "MANAGE_CHANNELS")]
pub async fn disable(ctx: Context<'_>) -> Result<(), Error> {
    translation::disable_channel(ctx).await
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

/// Show who's currently available
#[poise::command(slash_command, prefix_command, rename = "who")]
pub async fn who_cmd(ctx: Context<'_>) -> Result<(), Error> {
    status::who(ctx).await
}

/// Show full team status with time zones
#[poise::command(slash_command, prefix_command, rename = "team")]
pub async fn team_cmd(ctx: Context<'_>) -> Result<(), Error> {
    status::team(ctx).await
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
pub async fn team(ctx: Context<'_>) -> Result<(), Error> {
    status::team(ctx).await
}
