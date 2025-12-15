//! Translation module - Remove language barriers
//!
//! Handles:
//! - Auto-translation of non-English messages to English (public in channel)
//! - DM translations of English messages to subscribers in their preferred language

use crate::bot::{Context, Data, Error};
use crate::services::translator::TranslatorService;
use poise::serenity_prelude::{self as serenity, Message};
use tracing::{debug, error, info, warn};

/// Handle incoming messages for translation
pub async fn handle_message(
    ctx: &serenity::Context,
    message: &Message,
    data: &Data,
) -> Result<(), Error> {
    // Check if translation is enabled in this channel
    let channel_id = message.channel_id.to_string();
    if !data.db.is_translation_enabled(&channel_id).await? {
        return Ok(());
    }

    let content = message.content.trim();
    if content.is_empty() {
        return Ok(());
    }

    // Detect language
    let detected = whatlang::detect(content);
    let lang_code = detected
        .map(|info| info.lang().code())
        .unwrap_or("en");

    debug!("Detected language: {} for message: {}", lang_code, &content[..content.len().min(50)]);

    if lang_code == "eng" || lang_code == "en" {
        // English message - DM translations to subscribers
        handle_english_message(ctx, message, data).await?;
    } else {
        // Non-English message - translate to English publicly
        handle_non_english_message(ctx, message, data, lang_code).await?;
    }

    Ok(())
}

/// Handle English messages - DM translations to non-English subscribers
async fn handle_english_message(
    ctx: &serenity::Context,
    message: &Message,
    data: &Data,
) -> Result<(), Error> {
    // Get all translation subscribers
    // For now, we support Hindi (hi) as the primary non-English language
    let hindi_subscribers = data.db.get_subscribers_for_language("hi").await?;

    if hindi_subscribers.is_empty() {
        return Ok(());
    }

    // Don't translate short messages or bot commands
    if message.content.len() < 10 || message.content.starts_with('/') {
        return Ok(());
    }

    // Translate to Hindi
    let translator = TranslatorService::new(&data.config.translation);
    let translated = match translator.translate(&message.content, "en", "hi").await {
        Ok(t) => t,
        Err(e) => {
            warn!("Translation failed: {}", e);
            return Ok(());
        }
    };

    // Get channel name for context
    let channel_name = message
        .channel_id
        .name(ctx)
        .await
        .unwrap_or_else(|_| "channel".to_string());

    // DM each subscriber
    for subscriber_id in hindi_subscribers {
        // Don't DM the message author
        if subscriber_id == message.author.id.to_string() {
            continue;
        }

        // Parse user ID and send DM
        if let Ok(user_id) = subscriber_id.parse::<u64>() {
            let user = serenity::UserId::new(user_id);
            if let Ok(dm_channel) = user.create_dm_channel(ctx).await {
                let dm_content = format!(
                    "[#{}] **{}** said:\n{}",
                    channel_name,
                    message.author.name,
                    translated
                );
                if let Err(e) = dm_channel.say(ctx, dm_content).await {
                    debug!("Failed to DM user {}: {}", user_id, e);
                }
            }
        }
    }

    Ok(())
}

/// Handle non-English messages - translate to English publicly
async fn handle_non_english_message(
    ctx: &serenity::Context,
    message: &Message,
    data: &Data,
    source_lang: &str,
) -> Result<(), Error> {
    // Translate to English
    let translator = TranslatorService::new(&data.config.translation);
    let translated = match translator.translate(&message.content, source_lang, "en").await {
        Ok(t) => t,
        Err(e) => {
            warn!("Translation failed: {}", e);
            // Post warning emoji as reaction instead of failing silently
            if let Err(e) = message.react(ctx, '⚠').await {
                debug!("Failed to add reaction: {}", e);
            }
            return Ok(());
        }
    };

    // Post translation in channel
    let translation_msg = format!("🌐 **Translation:** {}", translated);
    if let Err(e) = message.reply(ctx, translation_msg).await {
        error!("Failed to post translation: {}", e);
    }

    Ok(())
}

// ==================== Commands ====================

/// Subscribe to translations
pub async fn subscribe(ctx: Context<'_>, language: String) -> Result<(), Error> {
    let lang_code = normalize_language(&language);

    if !is_supported_language(&lang_code) {
        ctx.say(format!(
            "Language '{}' is not currently supported. Supported: English (en), Hindi (hi)",
            language
        ))
        .await?;
        return Ok(());
    }

    let user_id = ctx.author().id.to_string();
    ctx.data().db.add_translation_subscription(&user_id, &lang_code).await?;

    info!("User {} subscribed to {} translations", user_id, lang_code);
    ctx.say(format!(
        "✅ You're now subscribed to translations in **{}**. \
         You'll receive DMs when English messages are posted in translation-enabled channels.",
        language_name(&lang_code)
    ))
    .await?;

    Ok(())
}

/// Unsubscribe from translations
pub async fn unsubscribe(ctx: Context<'_>) -> Result<(), Error> {
    let user_id = ctx.author().id.to_string();
    ctx.data().db.remove_translation_subscription(&user_id).await?;

    info!("User {} unsubscribed from translations", user_id);
    ctx.say("✅ You've been unsubscribed from translation DMs.").await?;

    Ok(())
}

/// Show translation status
pub async fn status(ctx: Context<'_>) -> Result<(), Error> {
    let user_id = ctx.author().id.to_string();
    let subscription = ctx.data().db.get_translation_subscription(&user_id).await?;

    match subscription {
        Some(ref lang) => {
            ctx.say(format!(
                "📖 You're subscribed to translations in **{}**.",
                language_name(lang)
            ))
            .await?;
        }
        None => {
            ctx.say("📖 You're not subscribed to any translations.").await?;
        }
    }

    Ok(())
}

/// Enable translation in channel
pub async fn enable_channel(ctx: Context<'_>) -> Result<(), Error> {
    let channel_id = ctx.channel_id().to_string();
    let enabled_by = ctx.author().id.to_string();

    ctx.data()
        .db
        .enable_translation_channel(&channel_id, &enabled_by)
        .await?;

    info!(
        "Translation enabled in channel {} by {}",
        channel_id, enabled_by
    );
    ctx.say("✅ Translation is now **enabled** in this channel.\n\
             • Non-English messages will be auto-translated to English\n\
             • English messages will be DM'd to subscribers in their language")
        .await?;

    Ok(())
}

/// Disable translation in channel
pub async fn disable_channel(ctx: Context<'_>) -> Result<(), Error> {
    let channel_id = ctx.channel_id().to_string();
    ctx.data().db.disable_translation_channel(&channel_id).await?;

    info!("Translation disabled in channel {}", channel_id);
    ctx.say("✅ Translation is now **disabled** in this channel.").await?;

    Ok(())
}

// ==================== Helpers ====================

/// Normalize language input to language code
fn normalize_language(input: &str) -> String {
    match input.to_lowercase().as_str() {
        "hindi" | "hi" | "hin" => "hi".to_string(),
        "english" | "en" | "eng" => "en".to_string(),
        "french" | "fr" | "fra" => "fr".to_string(),
        other => other.to_string(),
    }
}

/// Check if a language is supported
fn is_supported_language(code: &str) -> bool {
    matches!(code, "hi" | "en" | "fr")
}

/// Get human-readable language name
fn language_name(code: &str) -> &'static str {
    match code {
        "hi" => "Hindi",
        "en" => "English",
        "fr" => "French",
        _ => "Unknown",
    }
}
