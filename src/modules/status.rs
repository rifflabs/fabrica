//! Status module - Coordination visibility without surveillance
//!
//! Tracks who's available, busy, or away - completely self-reported
//! and under each person's control.

use crate::bot::{Context, Error};
use crate::db::UserStatus;
use chrono::Local;
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

    response.push_str("───────────────────────────────");

    ctx.say(response).await?;

    Ok(())
}

/// Show available team members with their schedule
pub async fn team(ctx: Context<'_>, public: bool) -> Result<(), Error> {
    let guild_id = match ctx.guild_id() {
        Some(gid) => gid.to_string(),
        None => {
            ctx.say("⚠️ Team status can only be viewed in a server.").await?;
            return Ok(());
        }
    };

    let available = ctx.data().db.get_users_by_status("available").await?;
    let busy = ctx.data().db.get_users_by_status("busy").await?;
    let viewer_id = ctx.author().id.to_string();
    let viewer_settings = ctx.data().db.get_user_settings(&viewer_id).await?;
    let today = Local::now().format("%Y-%m-%d").to_string();
    let now = chrono::Utc::now().timestamp();
    let fifteen_minutes = 15 * 60; // seconds

    let mut response = String::from("───────────────────────────────\n");
    let mut shown_count = 0;

    // Available users - always show
    if !available.is_empty() {
        response.push_str("🟢 **Available**\n");
        for status in &available {
            let member_settings = ctx.data().db.get_user_settings(&status.discord_id).await?;
            response.push_str(&format_team_member(&status, &member_settings, &viewer_settings, &guild_id, &today, ctx).await);
            shown_count += 1;
        }
        response.push('\n');
    }

    // Busy users - show if busy < 15 min OR always_show_me
    let visible_busy: Vec<_> = {
        let mut result = Vec::new();
        for status in &busy {
            let member_settings = ctx.data().db.get_user_settings(&status.discord_id).await?;
            let busy_duration = now - status.updated_at;
            if busy_duration < fifteen_minutes || member_settings.always_show_me {
                result.push((status, member_settings, busy_duration));
            }
        }
        result
    };

    if !visible_busy.is_empty() {
        response.push_str("🟡 **Busy**\n");
        for (status, member_settings, busy_duration) in &visible_busy {
            let mut line = format_team_member(status, member_settings, &viewer_settings, &guild_id, &today, ctx).await;
            // Add how long they've been busy
            let mins = busy_duration / 60;
            if mins > 0 {
                line = line.trim_end().to_string();
                line.push_str(&format!(" ({}m)\n", mins));
            }
            response.push_str(&line);
            shown_count += 1;
        }
        response.push('\n');
    }

    // Away users never shown in /team

    if shown_count == 0 {
        let msg = "No team members are currently visible.";
        if public {
            ctx.say(msg).await?;
        } else {
            ctx.send(poise::CreateReply::default().content(msg).ephemeral(true)).await?;
        }
        return Ok(());
    }

    response.push_str("───────────────────────────────");

    if public {
        ctx.say(response).await?;
    } else {
        ctx.send(poise::CreateReply::default().content(response).ephemeral(true)).await?;
    }

    Ok(())
}

/// Format a team member for display, showing their local time
async fn format_team_member(
    status: &crate::db::UserStatus,
    member_settings: &crate::db::UserSettings,
    viewer_settings: &crate::db::UserSettings,
    guild_id: &str,
    today: &str,
    ctx: Context<'_>,
) -> String {
    let user_mention = format!("<@{}>", status.discord_id);
    let mut line = format!("  {}", user_mention);

    // Show their local time with timezone context
    if let Ok(member_tz) = member_settings.timezone.parse::<chrono_tz::Tz>() {
        let local_time = chrono::Utc::now().with_timezone(&member_tz);
        let time_str = if viewer_settings.is_12h() {
            local_time.format("%-I:%M%P").to_string()
        } else {
            local_time.format("%H:%M").to_string()
        };

        // Check if viewer is in the same timezone
        let same_tz = if let Ok(viewer_tz) = viewer_settings.timezone.parse::<chrono_tz::Tz>() {
            member_tz == viewer_tz
        } else {
            false
        };

        if same_tz {
            line.push_str(&format!(" 🕐 {}", time_str));
        } else {
            // Show timezone abbreviation for different timezones
            let tz_abbr = local_time.format("%Z").to_string();
            line.push_str(&format!(" 🕐 {} {}", time_str, tz_abbr));
        }
    }

    if let Some(msg) = &status.message {
        line.push_str(&format!(" - {}", msg));
    }

    // Check for today's schedule override (until time)
    if let Ok(Some((_, end_time))) = ctx.data().db.get_schedule_override(guild_id, &status.discord_id, today).await {
        let formatted = format_time_for_user(&end_time, viewer_settings);
        line.push_str(&format!(" (until {})", formatted));
    }

    line.push('\n');
    line
}

/// Format a time string for display to a user based on their settings
fn format_time_for_user(time: &str, settings: &crate::db::UserSettings) -> String {
    // time is in HH:MM format
    if settings.is_12h() {
        // Convert to 12h format
        if let Some((h, m)) = time.split_once(':') {
            if let (Ok(hour), Ok(min)) = (h.parse::<u8>(), m.parse::<u8>()) {
                let (h12, ampm) = if hour == 0 {
                    (12, "am")
                } else if hour < 12 {
                    (hour, "am")
                } else if hour == 12 {
                    (12, "pm")
                } else {
                    (hour - 12, "pm")
                };
                return format!("{}:{:02}{}", h12, min, ampm);
            }
        }
    }
    time.to_string()
}

/// Show user settings
pub async fn show_settings(ctx: Context<'_>) -> Result<(), Error> {
    let user_id = ctx.author().id.to_string();
    let settings = ctx.data().db.get_user_settings(&user_id).await?;

    let format_display = if settings.is_12h() { "12-hour (am/pm)" } else { "24-hour" };
    let always_show_display = if settings.always_show_me { "Yes" } else { "No" };

    let response = format!(
        "⚙️ **Your Settings**\n\n\
         **Timezone:** {}\n\
         **Time format:** {}\n\
         **Always show me:** {}\n\n\
         Use `/fabrica settings timezone <zone>` to change timezone\n\
         Use `/fabrica settings format 24h` or `/fabrica settings format 12h` to change format\n\
         Use `/fabrica settings always-show-me` to toggle visibility in /team",
        settings.timezone,
        format_display,
        always_show_display
    );

    ctx.send(poise::CreateReply::default().content(response).ephemeral(true)).await?;
    Ok(())
}

/// Toggle always-show-me setting
pub async fn toggle_always_show_me(ctx: Context<'_>) -> Result<(), Error> {
    let user_id = ctx.author().id.to_string();
    let settings = ctx.data().db.get_user_settings(&user_id).await?;
    let new_value = !settings.always_show_me;

    ctx.data().db.set_user_always_show_me(&user_id, new_value).await?;

    let msg = if new_value {
        "✅ **Always show me** is now **ON**.\nYou'll appear in `/team` even when busy for a long time or away."
    } else {
        "✅ **Always show me** is now **OFF**.\nYou'll be hidden from `/team` when busy for more than 15 minutes or away."
    };

    info!("User {} set always_show_me to {}", user_id, new_value);
    ctx.send(poise::CreateReply::default().content(msg).ephemeral(true)).await?;
    Ok(())
}

/// Set user timezone (admins can set for others)
pub async fn set_timezone(ctx: Context<'_>, timezone: String, target_user: Option<poise::serenity_prelude::User>) -> Result<(), Error> {
    let caller_id = ctx.author().id.to_string();

    // Determine target user
    let (target_id, target_mention) = match target_user {
        Some(ref user) => {
            // Check if caller is admin
            if !is_global_admin(&ctx) {
                ctx.send(poise::CreateReply::default()
                    .content("⚠️ Only admins can set timezone for other users.")
                    .ephemeral(true)).await?;
                return Ok(());
            }
            (user.id.to_string(), format!("<@{}>", user.id))
        }
        None => (caller_id.clone(), "your".to_string()),
    };

    // Validate timezone using chrono-tz
    let tz_str = timezone.trim();
    if tz_str.parse::<chrono_tz::Tz>().is_err() {
        // Try common aliases
        let normalized = match tz_str.to_lowercase().as_str() {
            "london" | "uk" | "gmt" | "bst" => "Europe/London",
            "new york" | "nyc" | "est" | "edt" | "eastern" => "America/New_York",
            "la" | "los angeles" | "pst" | "pdt" | "pacific" => "America/Los_Angeles",
            "chicago" | "cst" | "cdt" | "central" => "America/Chicago",
            "denver" | "mst" | "mdt" | "mountain" => "America/Denver",
            "tokyo" | "jst" => "Asia/Tokyo",
            "paris" | "cet" | "cest" => "Europe/Paris",
            "berlin" => "Europe/Berlin",
            "sydney" | "aest" | "aedt" => "Australia/Sydney",
            "mumbai" | "ist" | "india" => "Asia/Kolkata",
            "dubai" | "gst" => "Asia/Dubai",
            "singapore" | "sgt" => "Asia/Singapore",
            "hong kong" | "hkt" => "Asia/Hong_Kong",
            "amsterdam" => "Europe/Amsterdam",
            "toronto" => "America/Toronto",
            "vancouver" => "America/Vancouver",
            "seattle" => "America/Los_Angeles",
            "san francisco" | "sf" => "America/Los_Angeles",
            _ => {
                ctx.send(poise::CreateReply::default()
                    .content(format!("⚠️ Unknown timezone: `{}`\n\nExamples: `London`, `New York`, `Tokyo`, `Europe/Paris`, `America/Los_Angeles`", tz_str))
                    .ephemeral(true)).await?;
                return Ok(());
            }
        };
        ctx.data().db.set_user_timezone(&target_id, normalized).await?;
        info!("User {} set timezone for {} to {} (from {})", caller_id, target_id, normalized, tz_str);
        let msg = if target_user.is_some() {
            format!("🌍 Set {} timezone to **{}**", target_mention, normalized)
        } else {
            format!("🌍 Timezone set to **{}**", normalized)
        };
        ctx.send(poise::CreateReply::default().content(msg).ephemeral(true)).await?;
    } else {
        ctx.data().db.set_user_timezone(&target_id, tz_str).await?;
        info!("User {} set timezone for {} to {}", caller_id, target_id, tz_str);
        let msg = if target_user.is_some() {
            format!("🌍 Set {} timezone to **{}**", target_mention, tz_str)
        } else {
            format!("🌍 Timezone set to **{}**", tz_str)
        };
        ctx.send(poise::CreateReply::default().content(msg).ephemeral(true)).await?;
    }

    Ok(())
}

/// Check if the user is a global admin
fn is_global_admin(ctx: &Context<'_>) -> bool {
    let user_id = ctx.author().id.to_string();
    ctx.data().config.discord.admin_ids.contains(&user_id)
}

/// Set user time format
pub async fn set_time_format(ctx: Context<'_>, format: String) -> Result<(), Error> {
    let user_id = ctx.author().id.to_string();
    let fmt = format.trim().to_lowercase();

    let normalized = match fmt.as_str() {
        "24h" | "24" | "24-hour" | "24hour" => "24h",
        "12h" | "12" | "12-hour" | "12hour" | "am-pm" | "ampm" | "am/pm" => "12h",
        _ => {
            ctx.send(poise::CreateReply::default()
                .content("⚠️ Invalid format. Use `24h` or `12h`")
                .ephemeral(true)).await?;
            return Ok(());
        }
    };

    ctx.data().db.set_user_time_format(&user_id, normalized).await?;
    info!("User {} set time format to {}", user_id, normalized);

    let display = if normalized == "12h" { "12-hour (am/pm)" } else { "24-hour" };
    ctx.send(poise::CreateReply::default()
        .content(format!("🕐 Time format set to **{}**", display))
        .ephemeral(true)).await?;

    Ok(())
}

/// Set working hours
/// Supports:
/// - "Mon,Tue,Wed,Thu,Fri 9:30 to 23:30"
/// - "M-F 9:30 to 23:30"
/// - "today 9:30 to 23:30"
/// - "today until 23:30"
/// - "until 23:30"
pub async fn set_hours(ctx: Context<'_>, schedule: String) -> Result<(), Error> {
    let guild_id = match ctx.guild_id() {
        Some(gid) => gid.to_string(),
        None => {
            ctx.say("⚠️ Hours can only be set in a server.").await?;
            return Ok(());
        }
    };
    let user_id = ctx.author().id.to_string();
    let schedule = schedule.trim();

    // Parse the schedule
    match parse_schedule(schedule) {
        Ok(ParsedSchedule::Weekly { days, start, end }) => {
            ctx.data().db.set_weekly_schedule(&guild_id, &user_id, &days, &start, &end).await?;

            let day_names = days_to_names(&days);
            info!("User {} set weekly schedule in guild {}: {} {}-{}", user_id, guild_id, day_names, start, end);
            ctx.say(format!("⏰ Set schedule: **{}** from **{}** to **{}**", day_names, start, end)).await?;
        }
        Ok(ParsedSchedule::TodayRange { start, end }) => {
            let today = Local::now().format("%Y-%m-%d").to_string();
            ctx.data().db.set_schedule_override(&guild_id, &user_id, &today, Some(&start), &end).await?;

            info!("User {} set today's schedule in guild {}: {}-{}", user_id, guild_id, start, end);
            ctx.say(format!("⏰ Set for today: **{}** to **{}**", start, end)).await?;
        }
        Ok(ParsedSchedule::TodayUntil { end }) => {
            let today = Local::now().format("%Y-%m-%d").to_string();
            ctx.data().db.set_schedule_override(&guild_id, &user_id, &today, None, &end).await?;

            info!("User {} set today until in guild {}: {}", user_id, guild_id, end);
            ctx.say(format!("⏰ Available today until **{}**", end)).await?;
        }
        Err(e) => {
            ctx.say(format!("⚠️ Couldn't parse schedule: {}\n\n\
                **Examples:**\n\
                • `/fabrica hours Mon,Tue,Wed,Thu,Fri 9:30 to 23:30`\n\
                • `/fabrica hours M-F 9:30 to 23:30`\n\
                • `/fabrica hours today 9:30 to 23:30`\n\
                • `/fabrica hours today until 23:30`\n\
                • `/fabrica hours until 23:30`", e)).await?;
        }
    }

    Ok(())
}

/// Show current hours
pub async fn show_hours(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = match ctx.guild_id() {
        Some(gid) => gid.to_string(),
        None => {
            ctx.say("⚠️ Hours can only be viewed in a server.").await?;
            return Ok(());
        }
    };
    let user_id = ctx.author().id.to_string();

    let weekly = ctx.data().db.get_weekly_schedule(&guild_id, &user_id).await?;
    let today = Local::now().format("%Y-%m-%d").to_string();
    let override_today = ctx.data().db.get_schedule_override(&guild_id, &user_id, &today).await?;

    let mut response = String::from("⏰ **Your Schedule**\n\n");

    if let Some((start, end)) = override_today {
        match start {
            Some(s) => response.push_str(&format!("**Today:** {} to {}\n\n", s, end)),
            None => response.push_str(&format!("**Today:** until {}\n\n", end)),
        }
    }

    if weekly.is_empty() {
        response.push_str("No weekly schedule set.");
    } else {
        response.push_str("**Weekly:**\n");
        for (day, start, end) in weekly {
            response.push_str(&format!("  {}: {} to {}\n", day_name(day), start, end));
        }
    }

    ctx.say(response).await?;
    Ok(())
}

// ==================== Parsing ====================

enum ParsedSchedule {
    Weekly { days: Vec<u8>, start: String, end: String },
    TodayRange { start: String, end: String },
    TodayUntil { end: String },
}

fn parse_schedule(input: &str) -> Result<ParsedSchedule, String> {
    let input = input.trim().to_lowercase();

    // "until HH:MM" - shorthand for "today until HH:MM"
    if input.starts_with("until ") {
        let time = input.strip_prefix("until ").unwrap().trim();
        let end = parse_time(time)?;
        return Ok(ParsedSchedule::TodayUntil { end });
    }

    // "today until HH:MM"
    if input.starts_with("today until ") {
        let time = input.strip_prefix("today until ").unwrap().trim();
        let end = parse_time(time)?;
        return Ok(ParsedSchedule::TodayUntil { end });
    }

    // "today HH:MM to HH:MM"
    if input.starts_with("today ") {
        let rest = input.strip_prefix("today ").unwrap().trim();
        let (start, end) = parse_time_range(rest)?;
        return Ok(ParsedSchedule::TodayRange { start, end });
    }

    // Weekly schedule: "DAYS HH:MM to HH:MM"
    // Find where the time part starts (look for a digit)
    let time_start = input.find(|c: char| c.is_ascii_digit())
        .ok_or("Could not find time in schedule")?;

    let days_part = input[..time_start].trim();
    let time_part = input[time_start..].trim();

    let days = parse_days(days_part)?;
    let (start, end) = parse_time_range(time_part)?;

    Ok(ParsedSchedule::Weekly { days, start, end })
}

fn parse_days(input: &str) -> Result<Vec<u8>, String> {
    let input = input.trim().to_lowercase();

    // Check for range format like "m-f"
    if input.contains('-') && !input.contains(',') {
        let parts: Vec<&str> = input.split('-').collect();
        if parts.len() == 2 {
            let start_day = parse_single_day(parts[0].trim())?;
            let end_day = parse_single_day(parts[1].trim())?;

            if start_day <= end_day {
                return Ok((start_day..=end_day).collect());
            } else {
                // Wrap around (e.g., Fri-Mon)
                let mut days: Vec<u8> = (start_day..=6).collect();
                days.extend(0..=end_day);
                return Ok(days);
            }
        }
    }

    // Comma-separated list
    let mut days = Vec::new();
    for part in input.split(',') {
        let day = parse_single_day(part.trim())?;
        if !days.contains(&day) {
            days.push(day);
        }
    }

    days.sort();
    Ok(days)
}

fn parse_single_day(input: &str) -> Result<u8, String> {
    match input.to_lowercase().as_str() {
        "m" | "mon" | "monday" => Ok(0),
        "tu" | "tue" | "tues" | "tuesday" => Ok(1),
        "w" | "wed" | "wednesday" => Ok(2),
        "th" | "thu" | "thur" | "thurs" | "thursday" => Ok(3),
        "f" | "fri" | "friday" => Ok(4),
        "sa" | "sat" | "saturday" => Ok(5),
        "su" | "sun" | "sunday" => Ok(6),
        _ => Err(format!("Unknown day: {}", input)),
    }
}

fn parse_time_range(input: &str) -> Result<(String, String), String> {
    // Look for "to" or "-" as separator
    let (start_str, end_str) = if input.contains(" to ") {
        let parts: Vec<&str> = input.split(" to ").collect();
        if parts.len() != 2 {
            return Err("Expected format: HH:MM to HH:MM".to_string());
        }
        (parts[0].trim(), parts[1].trim())
    } else if input.contains('-') && input.matches('-').count() == 1 {
        // Single dash, might be time separator
        let parts: Vec<&str> = input.split('-').collect();
        if parts.len() != 2 {
            return Err("Expected format: HH:MM-HH:MM".to_string());
        }
        (parts[0].trim(), parts[1].trim())
    } else {
        return Err("Expected format: HH:MM to HH:MM".to_string());
    };

    let start = parse_time(start_str)?;
    let end = parse_time(end_str)?;

    Ok((start, end))
}

fn parse_time(input: &str) -> Result<String, String> {
    let input = input.trim();
    let lower = input.to_lowercase();

    // Handle 12-hour format manually (5pm, 5:30pm, 5 pm, 5:30 pm, etc.)
    let (time_part, is_pm) = if lower.ends_with("pm") {
        (lower.trim_end_matches("pm").trim(), true)
    } else if lower.ends_with("am") {
        (lower.trim_end_matches("am").trim(), false)
    } else {
        // No AM/PM suffix - try 24-hour parsing
        if input.contains(':') {
            let parts: Vec<&str> = input.split(':').collect();
            if parts.len() == 2 {
                if let (Ok(h), Ok(m)) = (parts[0].parse::<u8>(), parts[1].parse::<u8>()) {
                    if h < 24 && m < 60 {
                        return Ok(format!("{:02}:{:02}", h, m));
                    }
                }
            }
        }
        // Try just a number as hour (e.g., "17" -> "17:00")
        if let Ok(h) = input.parse::<u8>() {
            if h < 24 {
                return Ok(format!("{:02}:00", h));
            }
        }
        return Err(format!("Invalid time format: {}", input));
    };

    // Parse the time part (could be "5", "5:30", etc.)
    let (hour, minute) = if time_part.contains(':') {
        let parts: Vec<&str> = time_part.split(':').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid time format: {}", input));
        }
        let h: u8 = parts[0].parse().map_err(|_| format!("Invalid hour: {}", parts[0]))?;
        let m: u8 = parts[1].parse().map_err(|_| format!("Invalid minute: {}", parts[1]))?;
        (h, m)
    } else {
        let h: u8 = time_part.parse().map_err(|_| format!("Invalid hour: {}", time_part))?;
        (h, 0)
    };

    // Validate and convert to 24-hour
    if hour > 12 || minute >= 60 {
        return Err(format!("Invalid time: {}", input));
    }

    let hour_24 = if is_pm {
        if hour == 12 { 12 } else { hour + 12 }
    } else {
        if hour == 12 { 0 } else { hour }
    };

    Ok(format!("{:02}:{:02}", hour_24, minute))
}

fn day_name(day: u8) -> &'static str {
    match day {
        0 => "Monday",
        1 => "Tuesday",
        2 => "Wednesday",
        3 => "Thursday",
        4 => "Friday",
        5 => "Saturday",
        6 => "Sunday",
        _ => "Unknown",
    }
}

fn days_to_names(days: &[u8]) -> String {
    // Check for common patterns
    if days == &[0, 1, 2, 3, 4] {
        return "Mon-Fri".to_string();
    }
    if days == &[0, 1, 2, 3, 4, 5, 6] {
        return "Every day".to_string();
    }
    if days == &[5, 6] {
        return "Sat-Sun".to_string();
    }

    // Otherwise list them
    let names: Vec<&str> = days.iter().map(|&d| {
        match d {
            0 => "Mon",
            1 => "Tue",
            2 => "Wed",
            3 => "Thu",
            4 => "Fri",
            5 => "Sat",
            6 => "Sun",
            _ => "?",
        }
    }).collect();

    names.join(", ")
}
