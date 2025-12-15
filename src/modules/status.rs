//! Status module - Coordination visibility without surveillance
//!
//! Tracks who's available, busy, or away - completely self-reported
//! and under each person's control.

use crate::bot::{Context, Error};
use crate::db::UserStatus;
use tracing::info;

/// Set status to available
pub async fn set_available(ctx: Context<'_>, message: Option<String>) -> Result<(), Error> {
    let user_id = ctx.author().id.to_string();
    let status = UserStatus::available(&user_id, message.clone());

    ctx.data().db.set_status(status).await?;

    let response = match message {
        Some(msg) => format!("🟢 You're now **available** - {}", msg),
        None => "🟢 You're now **available**".to_string(),
    };

    info!("User {} set status to available", user_id);
    ctx.say(response).await?;

    Ok(())
}

/// Set status to busy
pub async fn set_busy(ctx: Context<'_>, message: Option<String>) -> Result<(), Error> {
    let user_id = ctx.author().id.to_string();
    let status = UserStatus::busy(&user_id, message.clone());

    ctx.data().db.set_status(status).await?;

    let response = match message {
        Some(msg) => format!("🟡 You're now **busy** - {}", msg),
        None => "🟡 You're now **busy**".to_string(),
    };

    info!("User {} set status to busy", user_id);
    ctx.say(response).await?;

    Ok(())
}

/// Set status to away
pub async fn set_away(ctx: Context<'_>, message: Option<String>) -> Result<(), Error> {
    let user_id = ctx.author().id.to_string();
    let status = UserStatus::away(&user_id, message.clone());

    ctx.data().db.set_status(status).await?;

    let response = match message {
        Some(msg) => format!("🔴 You're now **away** - {}", msg),
        None => "🔴 You're now **away**".to_string(),
    };

    info!("User {} set status to away", user_id);
    ctx.say(response).await?;

    Ok(())
}

/// Clear status
pub async fn clear(ctx: Context<'_>) -> Result<(), Error> {
    let user_id = ctx.author().id.to_string();
    ctx.data().db.clear_status(&user_id).await?;

    info!("User {} cleared status", user_id);
    ctx.say("⚫ Your status has been cleared.").await?;

    Ok(())
}

/// Show who's available
pub async fn who(ctx: Context<'_>) -> Result<(), Error> {
    let available = ctx.data().db.get_users_by_status("available").await?;
    let busy = ctx.data().db.get_users_by_status("busy").await?;
    let away = ctx.data().db.get_users_by_status("away").await?;

    let mut response = String::from("───────────────────────────────\n");

    // Available
    response.push_str(&format!("🟢 **Available** ({})\n", available.len()));
    if available.is_empty() {
        response.push_str("  No one currently available\n");
    } else {
        for status in &available {
            let user_mention = format!("<@{}>", status.discord_id);
            match &status.message {
                Some(msg) => response.push_str(&format!("  {} - {}\n", user_mention, msg)),
                None => response.push_str(&format!("  {}\n", user_mention)),
            }
        }
    }

    response.push('\n');

    // Busy
    response.push_str(&format!("🟡 **Busy** ({})\n", busy.len()));
    if busy.is_empty() {
        response.push_str("  No one currently busy\n");
    } else {
        for status in &busy {
            let user_mention = format!("<@{}>", status.discord_id);
            match &status.message {
                Some(msg) => response.push_str(&format!("  {} - {}\n", user_mention, msg)),
                None => response.push_str(&format!("  {}\n", user_mention)),
            }
        }
    }

    response.push('\n');

    // Away
    response.push_str(&format!("🔴 **Away** ({})\n", away.len()));
    if away.is_empty() {
        response.push_str("  No one currently away\n");
    } else {
        for status in &away {
            let user_mention = format!("<@{}>", status.discord_id);
            match &status.message {
                Some(msg) => response.push_str(&format!("  {} - {}\n", user_mention, msg)),
                None => response.push_str(&format!("  {}\n", user_mention)),
            }
        }
    }

    response.push_str("───────────────────────────────");

    ctx.say(response).await?;

    Ok(())
}

/// Show full team status with time zones
pub async fn team(ctx: Context<'_>) -> Result<(), Error> {
    let all_statuses = ctx.data().db.get_all_statuses().await?;

    if all_statuses.is_empty() {
        ctx.say("No team members have set their status yet.\nUse `/fabrica available`, `/fabrica busy`, or `/fabrica away` to set yours!")
            .await?;
        return Ok(());
    }

    let mut response = String::from("───────────────────────────────\n");
    response.push_str("**Team Status**\n\n");

    // Group by timezone (simplified - just show all for now)
    // TODO: Proper timezone grouping

    for status in &all_statuses {
        let user_mention = format!("<@{}>", status.discord_id);
        let emoji = status.emoji();
        let label = status.label();

        response.push_str(&format!("{} {} - **{}**", emoji, user_mention, label));

        if let Some(msg) = &status.message {
            response.push_str(&format!(" - {}", msg));
        }

        if let Some(tz) = &status.timezone {
            response.push_str(&format!(" ({})", tz));
        }

        response.push('\n');
    }

    response.push_str("\n───────────────────────────────");

    ctx.say(response).await?;

    Ok(())
}
