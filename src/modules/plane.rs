//! Plane integration module - Project visibility at a glance
//!
//! Connects to Plane.so for project management visibility.

use crate::bot::{Context, Error};
use crate::db::WatchLevel;
use tracing::info;

/// Show project overview
pub async fn project(ctx: Context<'_>, name: String) -> Result<(), Error> {
    // TODO: Implement Plane API client
    // For now, return a placeholder
    ctx.say(format!(
        "📊 **{}**\n\n\
         ⚠️ Plane integration coming soon!\n\
         This will show project status, sprint progress, and open issues.",
        name
    ))
    .await?;

    Ok(())
}

/// List issues
pub async fn issues(
    ctx: Context<'_>,
    project: Option<String>,
    status_filter: Option<String>,
) -> Result<(), Error> {
    let project_name = project.unwrap_or_else(|| "all projects".to_string());
    let filter = status_filter.unwrap_or_else(|| "open".to_string());

    ctx.say(format!(
        "📋 **Issues for {}** (filter: {})\n\n\
         ⚠️ Plane integration coming soon!",
        project_name, filter
    ))
    .await?;

    Ok(())
}

/// Show sprint status
pub async fn sprint(ctx: Context<'_>, project: Option<String>) -> Result<(), Error> {
    let project_name = project.unwrap_or_else(|| "current".to_string());

    ctx.say(format!(
        "🏃 **Sprint Status** for {}\n\n\
         ⚠️ Plane integration coming soon!",
        project_name
    ))
    .await?;

    Ok(())
}

/// Watch a Plane project in this channel
pub async fn watch(ctx: Context<'_>, project: String, level: String) -> Result<(), Error> {
    let level = WatchLevel::from_str(&level).unwrap_or(WatchLevel::Important);
    let channel_id = ctx.channel_id().to_string();

    ctx.data()
        .db
        .set_plane_watch(&channel_id, &project, level.as_str())
        .await?;

    info!(
        "Channel {} now watching Plane project {} at level {:?}",
        channel_id, project, level
    );

    ctx.say(format!(
        "✅ This channel is now watching **{}** at **{}** level.\n\
         You'll receive notifications about project activity.",
        project,
        level.as_str()
    ))
    .await?;

    Ok(())
}

/// Stop watching a Plane project
pub async fn unwatch(ctx: Context<'_>, project: String) -> Result<(), Error> {
    let channel_id = ctx.channel_id().to_string();

    ctx.data()
        .db
        .remove_plane_watch(&channel_id, &project)
        .await?;

    info!(
        "Channel {} stopped watching Plane project {}",
        channel_id, project
    );

    ctx.say(format!(
        "✅ This channel is no longer watching **{}**.",
        project
    ))
    .await?;

    Ok(())
}
