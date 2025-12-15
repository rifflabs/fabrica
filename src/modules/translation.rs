//! Translation module - Remove language barriers
//!
//! Channel modes:
//! - off: No translation, no logging
//! - silent: DM translations only (including English - must subscribe)
//! - on: DM translations + public English translations
//! - transparent: All translations shown publicly in channel

use crate::bot::{Context, Data, Error};
use crate::services::translator::TranslatorService;
use poise::serenity_prelude::{self as serenity, Message, RoleId};
use tracing::{debug, error, info, warn};

/// Handle incoming messages for translation
pub async fn handle_message(
    ctx: &serenity::Context,
    message: &Message,
    data: &Data,
) -> Result<(), Error> {
    // Get guild ID - skip DMs
    let guild_id = match message.guild_id {
        Some(gid) => gid.to_string(),
        None => return Ok(()), // Skip DMs
    };

    // Get channel translation mode
    let channel_id = message.channel_id.to_string();
    let mode = data.db.get_channel_translation_mode(&guild_id, &channel_id).await?;

    // Off mode = no translation, no processing
    if mode == "off" {
        return Ok(());
    }

    let content = message.content.trim();
    if content.is_empty() {
        return Ok(());
    }

    // Detect language - use whatlang first, fall back to LLM if uncertain
    let detected = whatlang::detect(content);
    let (whatlang_code, confidence) = detected
        .map(|info| (info.lang().code(), info.confidence()))
        .unwrap_or(("unknown", 0.0));

    info!("whatlang detected: '{}' (confidence: {:.2}) for: {}", whatlang_code, confidence, truncate_str(content, 50));

    // If whatlang is confident, use its result
    let lang_code = if confidence >= 0.8 {
        // Convert 3-letter to 2-letter code
        match whatlang_code {
            "eng" => "en",
            "hin" => "hi",
            "fra" => "fr",
            "spa" => "es",
            "deu" => "de",
            other => other,
        }.to_string()
    } else {
        // Fall back to LLM for uncertain detections
        info!("whatlang uncertain, asking LLM...");
        let translator = TranslatorService::new(&data.config.translation);
        match translator.detect_language(content).await {
            Ok(code) => {
                info!("LLM detected language: '{}'", code);
                code
            }
            Err(e) => {
                warn!("LLM language detection failed: {}, defaulting to English", e);
                "en".to_string()
            }
        }
    };

    let is_english = lang_code == "en" || lang_code == "eng";

    if is_english {
        // English message - handle based on mode
        handle_english_message(ctx, message, data, &guild_id, &mode).await?;
    } else {
        // Non-English message - translate to English
        handle_non_english_message(ctx, message, data, &guild_id, &lang_code, &mode).await?;
    }

    Ok(())
}

/// Handle English messages - translate to other languages based on mode
async fn handle_english_message(
    ctx: &serenity::Context,
    message: &Message,
    data: &Data,
    guild_id: &str,
    mode: &str,
) -> Result<(), Error> {
    let channel_id = message.channel_id.to_string();
    info!("handle_english_message called (mode={}) for: {}", mode, truncate_str(&message.content, 50));

    // Don't translate bot commands
    if message.content.starts_with('/') {
        return Ok(());
    }

    let translator = TranslatorService::new(&data.config.translation);

    match mode {
        "transparent" => {
            // Get all languages subscribed to in this channel (excluding English)
            let all_languages = data.db.get_channel_subscribed_languages(guild_id, &channel_id).await?;
            debug!("All subscribed languages in channel {}: {:?}", channel_id, all_languages);

            let languages: Vec<String> = all_languages
                .into_iter()
                .filter(|l| l != "en")
                .collect();

            info!("Transparent mode: translating to {:?} for channel {}", languages, channel_id);

            // Translate to each language and post publicly
            let mut translations = Vec::new();
            for target_lang in languages {
                match translator.translate(&message.content, "en", &target_lang).await {
                    Ok(Some(translated)) => {
                        let lang_name = language_name(&target_lang);
                        translations.push(format!("**{}:** {}", lang_name, translated));
                    }
                    Ok(None) => {
                        debug!("No translation needed for {} -> {}", "en", target_lang);
                    }
                    Err(e) => {
                        warn!("Translation to {} failed: {}", target_lang, e);
                    }
                }
            }

            if !translations.is_empty() {
                let reply = format!("🌐 {}", translations.join("\n"));
                if let Err(e) = message.reply(ctx, reply).await {
                    error!("Failed to post translations: {}", e);
                }
            } else {
                debug!("No translations to post (no non-English subscriptions or all translations failed)");
            }
        }
        "silent" | "on" => {
            // Get non-English subscriptions for this channel
            let subscriptions = data.db.get_channel_non_english_subscriptions(guild_id, &channel_id).await?;
            if subscriptions.is_empty() {
                return Ok(());
            }

            // Group by language
            let mut by_language: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
            for (discord_id, language) in subscriptions {
                by_language.entry(language).or_default().push(discord_id);
            }

            let channel_name = message
                .channel_id
                .name(ctx)
                .await
                .unwrap_or_else(|_| "channel".to_string());

            // Translate and DM for each language
            for (target_lang, subscribers) in by_language {
                let translated = match translator.translate(&message.content, "en", &target_lang).await {
                    Ok(Some(t)) => t,
                    Ok(None) => continue,
                    Err(e) => {
                        warn!("Translation to {} failed: {}", target_lang, e);
                        continue;
                    }
                };

                for subscriber_id in &subscribers {
                    // Skip author unless debug mode
                    if subscriber_id == &message.author.id.to_string() {
                        let debug_mode = data.db.get_translation_debug_mode(guild_id, subscriber_id, &channel_id).await.unwrap_or(false);
                        if !debug_mode {
                            continue;
                        }
                    }

                    if let Ok(user_id) = subscriber_id.parse::<u64>() {
                        let user = serenity::UserId::new(user_id);
                        if let Ok(dm_channel) = user.create_dm_channel(ctx).await {
                            let dm_content = format!(
                                "[#{}] **{}** said:\n{}",
                                channel_name,
                                message.author.name,
                                translated
                            );
                            let _ = dm_channel.say(ctx, &dm_content).await;
                        }
                    }
                }
            }
        }
        _ => {}
    }

    Ok(())
}

/// Handle non-English messages - translate to English based on mode
async fn handle_non_english_message(
    ctx: &serenity::Context,
    message: &Message,
    data: &Data,
    guild_id: &str,
    source_lang: &str,
    mode: &str,
) -> Result<(), Error> {
    let channel_id = message.channel_id.to_string();
    info!("handle_non_english_message called (mode={}, lang={}) for: {}", mode, source_lang, truncate_str(&message.content, 50));

    // Translate to English
    let translator = TranslatorService::new(&data.config.translation);
    let translated = match translator.translate(&message.content, source_lang, "en").await {
        Ok(Some(t)) => t,
        Ok(None) => {
            debug!("No translation needed - text already in target language");
            return Ok(());
        }
        Err(e) => {
            warn!("Translation failed: {}", e);
            if let Err(e) = message.react(ctx, '⚠').await {
                debug!("Failed to add reaction: {}", e);
            }
            return Ok(());
        }
    };

    // Skip if translation is too similar to original (likely misdetected English)
    let original_lower = message.content.to_lowercase();
    let translated_lower = translated.to_lowercase();
    if original_lower == translated_lower || similarity(&original_lower, &translated_lower) > 0.85 {
        debug!("Skipping translation - too similar to original (likely misdetected language)");
        return Ok(());
    }

    match mode {
        "silent" => {
            // DM English subscribers for this channel only
            let en_subscribers = data.db.get_channel_subscribers_for_language(guild_id, &channel_id, "en").await?;
            let channel_name = message
                .channel_id
                .name(ctx)
                .await
                .unwrap_or_else(|_| "channel".to_string());

            for subscriber_id in en_subscribers {
                // Skip author unless debug mode
                if subscriber_id == message.author.id.to_string() {
                    let debug_mode = data.db.get_translation_debug_mode(guild_id, &subscriber_id, &channel_id).await.unwrap_or(false);
                    if !debug_mode {
                        continue;
                    }
                }

                if let Ok(user_id) = subscriber_id.parse::<u64>() {
                    let user = serenity::UserId::new(user_id);
                    if let Ok(dm_channel) = user.create_dm_channel(ctx).await {
                        let dm_content = format!(
                            "[#{}] **{}** said:\n{}",
                            channel_name,
                            message.author.name,
                            translated
                        );
                        let _ = dm_channel.say(ctx, &dm_content).await;
                    }
                }
            }
        }
        "on" | "transparent" => {
            // Post translation publicly in channel
            let translation_msg = format!("🌐 **Translation:** {}", translated);
            if let Err(e) = message.reply(ctx, translation_msg).await {
                error!("Failed to post translation: {}", e);
            }
        }
        _ => {}
    }

    Ok(())
}

// ==================== Commands ====================

/// Get guild_id from context, returning error message if in DM
fn get_guild_id(ctx: &Context<'_>) -> Option<String> {
    ctx.guild_id().map(|gid| gid.to_string())
}

/// Subscribe to translations in this channel (can subscribe to multiple languages)
pub async fn subscribe(ctx: Context<'_>, language: String) -> Result<(), Error> {
    let guild_id = match get_guild_id(&ctx) {
        Some(gid) => gid,
        None => {
            ctx.say("⚠️ Translation subscriptions are only available in servers.").await?;
            return Ok(());
        }
    };

    let lang_code = normalize_language(&language);

    if !is_supported_language(&lang_code) {
        ctx.say(format!(
            "Language '{}' is not currently supported.\n\
             Supported: English (en), Hindi (hi), French (fr), Spanish (es), German (de), Filipino (fil), Portuguese (pt), Korean (ko)",
            language
        ))
        .await?;
        return Ok(());
    }

    let user_id = ctx.author().id.to_string();
    let channel_id = ctx.channel_id().to_string();

    // Check if already subscribed
    if ctx.data().db.has_translation_subscription(&guild_id, &user_id, &channel_id, &lang_code).await? {
        ctx.say(format!(
            "📖 You're already subscribed to **{}** translations in this channel.",
            language_name(&lang_code)
        )).await?;
        return Ok(());
    }

    ctx.data().db.add_translation_subscription(&guild_id, &user_id, &channel_id, &lang_code).await?;

    info!("User {} subscribed to {} translations in channel {} (guild {})", user_id, lang_code, channel_id, guild_id);

    // Show all current subscriptions
    let all_subs = ctx.data().db.get_translation_subscriptions(&guild_id, &user_id, &channel_id).await?;
    let all_names: Vec<&str> = all_subs.iter().map(|l| language_name(l)).collect();

    let msg = format!(
        "✅ Subscribed to **{}** translations in this channel.\n\
         Your subscriptions: **{}**\n\n\
         ⚠️ *Translations are machine-generated and may contain inaccuracies.*",
        language_name(&lang_code),
        all_names.join(", ")
    );
    ctx.say(msg).await?;

    Ok(())
}

/// Unsubscribe from translations in this channel (optionally specify a language, or 'all' to remove all)
pub async fn unsubscribe(ctx: Context<'_>, language: Option<String>) -> Result<(), Error> {
    let guild_id = match get_guild_id(&ctx) {
        Some(gid) => gid,
        None => {
            ctx.say("⚠️ Translation subscriptions are only available in servers.").await?;
            return Ok(());
        }
    };

    let user_id = ctx.author().id.to_string();
    let channel_id = ctx.channel_id().to_string();

    match language {
        Some(lang) if lang.to_lowercase() == "all" => {
            ctx.data().db.remove_all_translation_subscriptions(&guild_id, &user_id, &channel_id).await?;
            info!("User {} unsubscribed from all translations in channel {} (guild {})", user_id, channel_id, guild_id);
            ctx.say("✅ Unsubscribed from all translation DMs in this channel.").await?;
        }
        Some(lang) => {
            let lang_code = normalize_language(&lang);
            if !ctx.data().db.has_translation_subscription(&guild_id, &user_id, &channel_id, &lang_code).await? {
                ctx.say(format!(
                    "📖 You're not subscribed to **{}** translations in this channel.",
                    language_name(&lang_code)
                )).await?;
                return Ok(());
            }

            ctx.data().db.remove_translation_subscription(&guild_id, &user_id, &channel_id, &lang_code).await?;
            info!("User {} unsubscribed from {} translations in channel {} (guild {})", user_id, lang_code, channel_id, guild_id);

            // Show remaining subscriptions
            let remaining = ctx.data().db.get_translation_subscriptions(&guild_id, &user_id, &channel_id).await?;
            if remaining.is_empty() {
                ctx.say(format!(
                    "✅ Unsubscribed from **{}** translations. You have no remaining subscriptions in this channel.",
                    language_name(&lang_code)
                )).await?;
            } else {
                let names: Vec<&str> = remaining.iter().map(|l| language_name(l)).collect();
                ctx.say(format!(
                    "✅ Unsubscribed from **{}** translations.\n\
                     Remaining subscriptions: **{}**",
                    language_name(&lang_code),
                    names.join(", ")
                )).await?;
            }
        }
        None => {
            // No language specified - show current subscriptions and ask for clarification
            let subs = ctx.data().db.get_translation_subscriptions(&guild_id, &user_id, &channel_id).await?;
            if subs.is_empty() {
                ctx.say("📖 You have no translation subscriptions in this channel.").await?;
            } else {
                let names: Vec<&str> = subs.iter().map(|l| language_name(l)).collect();
                ctx.say(format!(
                    "📖 Your subscriptions: **{}**\n\
                     To unsubscribe, use `/fabrica translate unsubscribe <language>` or `all` to remove all.",
                    names.join(", ")
                )).await?;
            }
        }
    }

    Ok(())
}

/// Show translation status for this channel
pub async fn status(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = match get_guild_id(&ctx) {
        Some(gid) => gid,
        None => {
            ctx.say("⚠️ Translation subscriptions are only available in servers.").await?;
            return Ok(());
        }
    };

    let user_id = ctx.author().id.to_string();
    let channel_id = ctx.channel_id().to_string();
    let subscriptions = ctx.data().db.get_translation_subscriptions(&guild_id, &user_id, &channel_id).await?;
    let debug_mode = ctx.data().db.get_translation_debug_mode(&guild_id, &user_id, &channel_id).await.unwrap_or(false);
    let channel_mode = ctx.data().db.get_channel_translation_mode(&guild_id, &channel_id).await?;

    if subscriptions.is_empty() {
        ctx.say(format!(
            "📖 You have no translation subscriptions in this channel.\n\
             Channel mode: **{}**",
            channel_mode
        )).await?;
    } else {
        let names: Vec<&str> = subscriptions.iter().map(|l| language_name(l)).collect();
        let debug_status = if debug_mode { "\n🔧 Debug mode: **ON**" } else { "" };
        ctx.say(format!(
            "📖 Your subscriptions: **{}**\n\
             Channel mode: **{}**{}",
            names.join(", "),
            channel_mode,
            debug_status
        )).await?;
    }

    Ok(())
}

/// Toggle debug mode (receive translations of your own messages) for this channel
pub async fn debug(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = match get_guild_id(&ctx) {
        Some(gid) => gid,
        None => {
            ctx.say("⚠️ Translation subscriptions are only available in servers.").await?;
            return Ok(());
        }
    };

    let user_id = ctx.author().id.to_string();
    let channel_id = ctx.channel_id().to_string();

    // Check if subscribed first
    let subscriptions = ctx.data().db.get_translation_subscriptions(&guild_id, &user_id, &channel_id).await?;
    if subscriptions.is_empty() {
        ctx.say("⚠️ You need to subscribe to translations first with `/fabrica translate subscribe <language>`").await?;
        return Ok(());
    }

    let current = ctx.data().db.get_translation_debug_mode(&guild_id, &user_id, &channel_id).await.unwrap_or(false);
    let new_state = !current;
    ctx.data().db.set_translation_debug_mode(&guild_id, &user_id, &channel_id, new_state).await?;

    if new_state {
        info!("User {} enabled translation debug mode in channel {} (guild {})", user_id, channel_id, guild_id);
        ctx.say("🔧 Debug mode **ON** - You'll receive DM translations of your own messages in this channel.").await?;
    } else {
        info!("User {} disabled translation debug mode in channel {} (guild {})", user_id, channel_id, guild_id);
        ctx.say("🔧 Debug mode **OFF** - You won't receive translations of your own messages in this channel.").await?;
    }

    Ok(())
}

/// Set translation mode for channel
pub async fn set_mode(ctx: Context<'_>, mode: String) -> Result<(), Error> {
    let guild_id = match get_guild_id(&ctx) {
        Some(gid) => gid,
        None => {
            ctx.say("⚠️ Translation settings are only available in servers.").await?;
            return Ok(());
        }
    };

    // Check if user has permission (guild role or MANAGE_CHANNELS)
    if !has_translation_permission(&ctx, &guild_id, "mode").await {
        ctx.say("⚠️ You need a configured role or MANAGE_CHANNELS permission to change translation settings.\n\
                 Server admins can configure roles with `/fabrica server allow mode @role`").await?;
        return Ok(());
    }

    let mode_lower = mode.to_lowercase();
    if !matches!(mode_lower.as_str(), "off" | "silent" | "on" | "transparent") {
        ctx.say("⚠️ Invalid mode. Available modes:\n\
                 • **off** - No translation\n\
                 • **silent** - DM translations only (subscribe to `en` for English translations)\n\
                 • **on** - DM translations + public English translations\n\
                 • **transparent** - All translations shown publicly").await?;
        return Ok(());
    }

    let channel_id = ctx.channel_id().to_string();
    let set_by = ctx.author().id.to_string();
    ctx.data().db.set_channel_translation_mode(&guild_id, &channel_id, &mode_lower, &set_by).await?;

    info!("Channel {} translation mode set to {} by {} (guild {})", channel_id, mode_lower, set_by, guild_id);

    let description = match mode_lower.as_str() {
        "off" => "Translation is **disabled**. Messages will not be processed.",
        "silent" => "Translation mode: **silent**\n\
                     • Non-English → English: DM to English subscribers only\n\
                     • English → Other: DM to language subscribers",
        "on" => "Translation mode: **on**\n\
                 • Non-English → English: Posted publicly\n\
                 • English → Other: DM to language subscribers",
        "transparent" => "Translation mode: **transparent**\n\
                         • All translations posted publicly in channel",
        _ => "Mode set.",
    };

    ctx.say(format!("✅ {}", description)).await?;
    Ok(())
}

/// Show current channel translation mode
pub async fn show_mode(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = match get_guild_id(&ctx) {
        Some(gid) => gid,
        None => {
            ctx.say("⚠️ Translation settings are only available in servers.").await?;
            return Ok(());
        }
    };

    let channel_id = ctx.channel_id().to_string();
    let mode = ctx.data().db.get_channel_translation_mode(&guild_id, &channel_id).await?;

    let description = match mode.as_str() {
        "off" => "**off** - No translation",
        "silent" => "**silent** - DM translations only",
        "on" => "**on** - DM + public English translations",
        "transparent" => "**transparent** - All translations public",
        _ => &mode,
    };

    ctx.say(format!("📖 Channel translation mode: {}", description)).await?;
    Ok(())
}

/// Check if user is a global admin (configured in fabrica.toml)
fn is_global_admin(ctx: &Context<'_>) -> bool {
    let user_id = ctx.author().id.to_string();
    ctx.data().config.discord.admin_ids.contains(&user_id)
}

/// Check if user has a configured role permission or MANAGE_CHANNELS permission
async fn has_translation_permission(ctx: &Context<'_>, guild_id: &str, permission: &str) -> bool {
    // Global admins bypass all permission checks
    if is_global_admin(ctx) {
        return true;
    }

    // Check for MANAGE_CHANNELS permission (always grants access)
    if let Some(member) = ctx.author_member().await {
        if let Ok(perms) = member.permissions(ctx) {
            if perms.manage_channels() {
                return true;
            }
        }
    }

    // Check if "everyone" has this permission
    if let Ok(allowed_roles) = ctx.data().db.get_roles_with_permission(guild_id, permission).await {
        if allowed_roles.iter().any(|r| r == "everyone") {
            return true;
        }

        // Check for configured role permission
        if let Some(member) = ctx.author_member().await {
            for role_id_str in &allowed_roles {
                if let Ok(role_id) = role_id_str.parse::<u64>() {
                    if member.roles.contains(&RoleId::new(role_id)) {
                        return true;
                    }
                }
            }
        }
    }

    // Check for admin permission (grants all permissions)
    if let Ok(admin_roles) = ctx.data().db.get_roles_with_permission(guild_id, "admin").await {
        // Check if "everyone" has admin
        if admin_roles.iter().any(|r| r == "everyone") {
            return true;
        }

        if let Some(member) = ctx.author_member().await {
            for role_id_str in admin_roles {
                if let Ok(role_id) = role_id_str.parse::<u64>() {
                    if member.roles.contains(&RoleId::new(role_id)) {
                        return true;
                    }
                }
            }
        }
    }

    false
}

/// Show recent messages translated to user's subscribed language
pub async fn last(ctx: Context<'_>, count: Option<u32>) -> Result<(), Error> {
    let guild_id = match get_guild_id(&ctx) {
        Some(gid) => gid,
        None => {
            ctx.say("⚠️ This command is only available in servers.").await?;
            return Ok(());
        }
    };

    let user_id = ctx.author().id.to_string();
    let channel_id = ctx.channel_id().to_string();

    // Get user's subscribed languages
    let subscriptions = ctx.data().db.get_translation_subscriptions(&guild_id, &user_id, &channel_id).await?;
    if subscriptions.is_empty() {
        ctx.say("⚠️ You need to subscribe to a language first with `/fabrica translate subscribe <language>`").await?;
        return Ok(());
    }

    // Use first non-English subscription, or English if that's all they have
    let target_lang = subscriptions.iter()
        .find(|l| *l != "en")
        .unwrap_or_else(|| subscriptions.first().unwrap())
        .clone();

    // Defer the reply since this might take a while
    ctx.defer().await?;

    // Get last usage info
    let last_usage = ctx.data().db.get_last_command_usage(&guild_id, &channel_id, &user_id).await?;
    let after_message_id = last_usage.and_then(|(_, msg_id)| msg_id);

    // Determine how many messages to fetch
    let limit = count.unwrap_or(50).min(100) as u8;

    // Get bot's own user ID to filter out its messages
    let bot_id = ctx.framework().bot_id;

    // Fetch messages from Discord
    let messages = if let Some(after_id) = after_message_id {
        // Fetch messages after the last one we showed
        if let Ok(msg_id) = after_id.parse::<u64>() {
            ctx.channel_id()
                .messages(ctx, serenity::GetMessages::new().after(serenity::MessageId::new(msg_id)).limit(limit))
                .await
                .unwrap_or_default()
        } else {
            ctx.channel_id()
                .messages(ctx, serenity::GetMessages::new().limit(limit))
                .await
                .unwrap_or_default()
        }
    } else if count.is_some() {
        // User specified count, just get last N
        ctx.channel_id()
            .messages(ctx, serenity::GetMessages::new().limit(limit))
            .await
            .unwrap_or_default()
    } else {
        // First time using command with no count - just get last 20
        ctx.channel_id()
            .messages(ctx, serenity::GetMessages::new().limit(20))
            .await
            .unwrap_or_default()
    };

    if messages.is_empty() {
        ctx.say("📭 No new messages to show.").await?;
        return Ok(());
    }

    // Filter out bot messages that are translations (start with 🌐)
    let filtered_messages: Vec<_> = messages.iter()
        .filter(|m| {
            // Skip bot's own translation messages
            if m.author.id == bot_id && m.content.starts_with("🌐") {
                return false;
            }
            // Skip empty messages
            if m.content.trim().is_empty() {
                return false;
            }
            true
        })
        .collect();

    if filtered_messages.is_empty() {
        ctx.say("📭 No messages to translate.").await?;
        return Ok(());
    }

    // Reverse to chronological order (Discord returns newest first)
    let mut chronological: Vec<_> = filtered_messages.into_iter().collect();
    chronological.reverse();

    let translator = TranslatorService::new(&ctx.data().config.translation);
    let target_lang_name = language_name(&target_lang);

    // Build the translated output
    let mut output = format!("📜 **Last {} messages translated to {}:**\n\n", chronological.len(), target_lang_name);
    let mut translations_added = 0;

    for msg in &chronological {
        // Format timestamp
        let timestamp = msg.timestamp.format("%H:%M");
        let author_name = &msg.author.name;
        let content = msg.content.trim();

        // Skip very short messages or just mentions/emojis
        if content.len() < 2 {
            continue;
        }

        // Detect source language
        let detected = whatlang::detect(content);
        let source_lang = detected
            .map(|info| {
                let code = info.lang().code();
                match code {
                    "eng" => "en",
                    "hin" => "hi",
                    "fra" => "fr",
                    "spa" => "es",
                    "deu" => "de",
                    other => other,
                }
            })
            .unwrap_or("en");

        // Translate if needed
        let translated_content = if source_lang == target_lang {
            content.to_string()
        } else {
            match translator.translate(content, source_lang, &target_lang).await {
                Ok(Some(t)) => t,
                Ok(None) => content.to_string(),
                Err(_) => content.to_string(),
            }
        };

        // Add to output
        output.push_str(&format!("[{}] **{}**: {}\n", timestamp, author_name, translated_content));
        translations_added += 1;

        // Check if we're approaching Discord's message limit (2000 chars)
        if output.len() > 1800 {
            output.push_str("\n_...truncated due to length_");
            break;
        }
    }

    if translations_added == 0 {
        ctx.say("📭 No translatable messages found.").await?;
        return Ok(());
    }

    // Update last usage with the newest message ID
    let newest_message_id = chronological.last().map(|m| m.id.to_string());
    ctx.data().db.set_last_command_usage(
        &guild_id,
        &channel_id,
        &user_id,
        newest_message_id.as_deref()
    ).await?;

    ctx.say(output).await?;

    info!("User {} used /fabrica last in channel {} (guild {}), showed {} messages in {}",
          user_id, channel_id, guild_id, translations_added, target_lang);

    Ok(())
}

/// Check if user has admin permission for server management
pub async fn has_admin_permission(ctx: &Context<'_>, guild_id: &str) -> bool {
    // Global admins bypass all permission checks
    if is_global_admin(ctx) {
        return true;
    }

    // Server admin (ADMINISTRATOR permission) always has access
    if let Some(member) = ctx.author_member().await {
        if let Ok(perms) = member.permissions(ctx) {
            if perms.administrator() {
                return true;
            }
        }
    }

    // Check for admin role permission
    if let Ok(admin_roles) = ctx.data().db.get_roles_with_permission(guild_id, "admin").await {
        // Check if "everyone" has admin
        if admin_roles.iter().any(|r| r == "everyone") {
            return true;
        }

        if let Some(member) = ctx.author_member().await {
            for role_id_str in admin_roles {
                if let Ok(role_id) = role_id_str.parse::<u64>() {
                    if member.roles.contains(&RoleId::new(role_id)) {
                        return true;
                    }
                }
            }
        }
    }

    false
}

// ==================== Helpers ====================

/// Normalize language input to language code
fn normalize_language(input: &str) -> String {
    match input.to_lowercase().as_str() {
        // English
        "english" | "en" | "eng" => "en".to_string(),
        // Hindi
        "hindi" | "hi" | "hin" => "hi".to_string(),
        // French
        "french" | "fr" | "fra" | "francais" | "français" => "fr".to_string(),
        // Spanish
        "spanish" | "es" | "spa" | "espanol" | "español" => "es".to_string(),
        // German
        "german" | "de" | "deu" | "deutsch" => "de".to_string(),
        // Filipino/Tagalog
        "filipino" | "fil" | "tagalog" | "tl" | "tgl" => "fil".to_string(),
        // Brazilian Portuguese
        "portuguese" | "pt" | "por" | "brazilian" | "pt-br" | "ptbr" | "portugues" | "português" => "pt".to_string(),
        // Korean
        "korean" | "ko" | "kor" | "한국어" => "ko".to_string(),
        other => other.to_string(),
    }
}

/// Check if a language is supported
fn is_supported_language(code: &str) -> bool {
    matches!(code, "en" | "hi" | "fr" | "es" | "de" | "fil" | "pt" | "ko")
}

/// Get human-readable language name
fn language_name(code: &str) -> &'static str {
    match code {
        "en" => "English",
        "hi" => "Hindi",
        "fr" => "French",
        "es" => "Spanish",
        "de" => "German",
        "fil" => "Filipino",
        "pt" => "Portuguese",
        "ko" => "Korean",
        _ => "Unknown",
    }
}

/// Truncate a string to at most n characters (UTF-8 safe)
fn truncate_str(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

/// Calculate similarity ratio between two strings (0.0 to 1.0)
fn similarity(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let a_words: Vec<&str> = a.split_whitespace().collect();
    let b_words: Vec<&str> = b.split_whitespace().collect();

    let matching = a_words.iter().filter(|w| b_words.contains(w)).count();
    let total = a_words.len().max(b_words.len());

    if total == 0 {
        return 0.0;
    }

    matching as f64 / total as f64
}
