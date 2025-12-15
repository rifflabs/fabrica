//! GitHub integration module - Git activity visibility
//!
//! Shows commits, PRs, and repository status.

use crate::bot::{Context, Error};
use crate::db::WatchLevel;
use tracing::info;

/// Show repository status
pub async fn repo(ctx: Context<'_>, name: String) -> Result<(), Error> {
    // TODO: Implement GitHub API client
    ctx.say(format!(
        "📦 **Repository: {}**\n\n\
         ⚠️ GitHub integration coming soon!\n\
         This will show repo stats, recent activity, and open PRs.",
        name
    ))
    .await?;

    Ok(())
}

/// Show recent commits
pub async fn commits(ctx: Context<'_>, repo: String, count: Option<u32>) -> Result<(), Error> {
    let count = count.unwrap_or(5);

    ctx.say(format!(
        "📝 **Recent {} commits for {}**\n\n\
         ⚠️ GitHub integration coming soon!",
        count, repo
    ))
    .await?;

    Ok(())
}

/// Show open pull requests
pub async fn prs(ctx: Context<'_>, repo: String) -> Result<(), Error> {
    ctx.say(format!(
        "🔀 **Open PRs for {}**\n\n\
         ⚠️ GitHub integration coming soon!",
        repo
    ))
    .await?;

    Ok(())
}

/// Watch a GitHub repo in this channel
pub async fn watch(ctx: Context<'_>, repo: String, level: String) -> Result<(), Error> {
    let level = WatchLevel::from_str(&level).unwrap_or(WatchLevel::Important);
    let channel_id = ctx.channel_id().to_string();

    ctx.data()
        .db
        .set_github_watch(&channel_id, &repo, level.as_str())
        .await?;

    info!(
        "Channel {} now watching GitHub repo {} at level {:?}",
        channel_id, repo, level
    );

    ctx.say(format!(
        "✅ This channel is now watching **{}** at **{}** level.\n\
         You'll receive notifications about:\n\
         {}",
        repo,
        level.as_str(),
        level_description(&level)
    ))
    .await?;

    Ok(())
}

/// Stop watching a GitHub repo
pub async fn unwatch(ctx: Context<'_>, repo: String) -> Result<(), Error> {
    let channel_id = ctx.channel_id().to_string();

    ctx.data()
        .db
        .remove_github_watch(&channel_id, &repo)
        .await?;

    info!(
        "Channel {} stopped watching GitHub repo {}",
        channel_id, repo
    );

    ctx.say(format!(
        "✅ This channel is no longer watching **{}**.",
        repo
    ))
    .await?;

    Ok(())
}

/// Get description of what a watch level includes
fn level_description(level: &WatchLevel) -> &'static str {
    match level {
        WatchLevel::All => "• All pushes, PRs, issues, comments, and releases",
        WatchLevel::Important => "• PRs (opened, merged, closed)\n• Releases\n• Milestones",
        WatchLevel::Minimal => "• Releases only\n• Merged PRs",
        WatchLevel::Off => "• Nothing (muted)",
    }
}
