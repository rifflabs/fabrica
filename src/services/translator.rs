//! Translation service - Routes to Palace Translator or other backends
//!
//! Uses cheap LLMs (Mistral, Devstral) for translation to minimize costs.
//! Returns None when no translation is needed (text already in target language).

use crate::config::TranslationConfig;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, warn};

/// Translation service that routes to configured backend
pub struct TranslatorService {
    config: TranslationConfig,
    client: reqwest::Client,
}

#[derive(Serialize)]
struct PalaceRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: u32,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct PalaceResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: MessageContent,
}

#[derive(Deserialize)]
struct MessageContent {
    content: String,
}

impl TranslatorService {
    /// Create a new translator service
    pub fn new(config: &TranslationConfig) -> Self {
        Self {
            config: config.clone(),
            client: reqwest::Client::new(),
        }
    }

    /// Translate text from one language to another
    /// Returns None if text is already in the target language (no translation needed)
    pub async fn translate(&self, text: &str, from: &str, to: &str) -> Result<Option<String>> {
        self.translate_with_dialect(text, from, to, None).await
    }

    /// Translate text from one language to another with optional dialect
    /// Returns None if text is already in the target language (no translation needed)
    pub async fn translate_with_dialect(&self, text: &str, from: &str, to: &str, dialect: Option<&str>) -> Result<Option<String>> {
        match self.config.backend.as_str() {
            "openrouter" => self.translate_via_openrouter_with_dialect(text, from, to, dialect).await,
            "direct" => self.translate_direct(text, from, to).await,
            other => {
                warn!("Unknown translation backend: {}, falling back to direct", other);
                self.translate_direct(text, from, to).await
            }
        }
    }

    /// Translate using OpenRouter API with optional dialect support
    /// Returns None if text is already in the target language
    async fn translate_via_openrouter_with_dialect(&self, text: &str, from: &str, to: &str, dialect: Option<&str>) -> Result<Option<String>> {
        let from_name = language_name(from);
        let to_name = language_name(to);

        let script_info = match to {
            "fil" => "Filipino uses the LATIN ALPHABET (same as English). Example: 'Magandang araw' not any Asian script.",
            "hi" => "Hindi uses DEVANAGARI script. Example: 'नमस्ते'",
            "fr" => "French uses the LATIN ALPHABET. Example: 'Bonjour'",
            "es" => "Spanish uses the LATIN ALPHABET. Example: 'Hola'",
            "de" => "German uses the LATIN ALPHABET. Example: 'Guten Tag'",
            "pt" => "Portuguese uses the LATIN ALPHABET. Example: 'Olá'",
            "en" => "English uses the LATIN ALPHABET.",
            "ko" => "Korean uses HANGUL script. Example: '안녕하세요' (annyeonghaseyo = hello)",
            _ => "Use the standard script for this language.",
        };

        // Build dialect instruction if specified
        let dialect_info = if let Some(d) = dialect {
            format!("\nDIALECT: Use the {} dialect/variety of {}. Use vocabulary, expressions, and phrasing natural to {} speakers.\n", d, to_name, d)
        } else {
            String::new()
        };

        let target_desc = if let Some(d) = dialect {
            format!("{} ({})", to_name, d)
        } else {
            to_name.to_string()
        };

        let prompt = format!(
            "You are a professional translator. Translate the following text from {} to {}.\n\n\
             SCRIPT INFORMATION: {}{}\n\n\
             STRICT RULES:\n\
             - Provide an ACCURATE, LITERAL translation\n\
             - Use the CORRECT script/alphabet as specified above\n\
             - Do NOT be creative, funny, or add interpretations\n\
             - For slang/internet terms (like 'LOL'), translate to the natural equivalent in {}\n\
             - If the text is ALREADY in {}, respond with EXACTLY: NO_TRANSLATION_NEEDED\n\
             - For untranslatable text (onomatopoeia like 'hmm', '...', sounds), output the original unchanged\n\
             - Output ONLY the translation - no explanations, notes, commentary, or extra text\n\
             - NEVER prefix with 'Translation:' or similar - just output the translated text directly\n\n\
             Text to translate:\n{}",
            from_name, target_desc, script_info, dialect_info, to_name, to_name, text
        );

        #[derive(Serialize)]
        struct OpenRouterRequest {
            model: String,
            messages: Vec<Message>,
            max_tokens: u32,
        }

        let request = OpenRouterRequest {
            model: self.config.model.clone(),
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt,
            }],
            max_tokens: 2048,
        };

        let url = format!("{}/chat/completions", self.config.openrouter_url);

        debug!("Translating via OpenRouter: {} -> {}", from, to);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.openrouter_api_key))
            .header("Content-Type", "application/json")
            .header("X-Title", "Palace Fabrica")
            .json(&request)
            .send()
            .await
            .context("Failed to send OpenRouter translation request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("OpenRouter API error - Status: {}, Body: {}", status, body);
            anyhow::bail!("OpenRouter translation request failed: {} - {}", status, body);
        }

        let result: PalaceResponse = response
            .json()
            .await
            .context("Failed to parse OpenRouter translation response")?;

        let translation = result
            .choices
            .first()
            .map(|c| c.message.content.trim().to_string())
            .unwrap_or_default();

        if translation.is_empty() {
            anyhow::bail!("Empty translation response from OpenRouter");
        }

        // Check if model indicated no translation needed
        if translation.contains("NO_TRANSLATION_NEEDED") {
            debug!("No translation needed for text: {}", truncate_str(text, 50));
            return Ok(None);
        }

        Ok(Some(translation))
    }

    /// Direct translation using simple word substitution (fallback)
    async fn translate_direct(&self, text: &str, _from: &str, _to: &str) -> Result<Option<String>> {
        // This is a placeholder - in production, you'd use a proper translation API
        // For now, just return the original text with a note
        warn!("Direct translation not implemented, returning original");
        Ok(Some(format!("[Translation unavailable] {}", text)))
    }

    /// Detect language using LLM
    pub async fn detect_language(&self, text: &str) -> Result<String> {
        let prompt = format!(
            "What language is this text written in? Respond with ONLY the ISO 639-1 two-letter language code (e.g., 'en' for English, 'hi' for Hindi, 'fr' for French, 'es' for Spanish, 'de' for German, etc.).\n\nText: {}",
            text
        );

        #[derive(Serialize)]
        struct OpenRouterRequest {
            model: String,
            messages: Vec<Message>,
            max_tokens: u32,
        }

        let request = OpenRouterRequest {
            model: self.config.model.clone(),
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt,
            }],
            max_tokens: 10,
        };

        let url = format!("{}/chat/completions", self.config.openrouter_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.openrouter_api_key))
            .header("Content-Type", "application/json")
            .header("X-Title", "Palace Fabrica")
            .json(&request)
            .send()
            .await
            .context("Failed to send language detection request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("OpenRouter API error - Status: {}, Body: {}", status, body);
            anyhow::bail!("Language detection request failed: {} - {}", status, body);
        }

        let result: PalaceResponse = response
            .json()
            .await
            .context("Failed to parse language detection response")?;

        let lang = result
            .choices
            .first()
            .map(|c| c.message.content.trim().to_lowercase())
            .unwrap_or_else(|| "en".to_string());

        // Clean up response - extract just the language code
        let lang_code = lang
            .chars()
            .filter(|c| c.is_alphabetic())
            .take(2)
            .collect::<String>();

        debug!("LLM detected language: {}", lang_code);
        Ok(if lang_code.is_empty() { "en".to_string() } else { lang_code })
    }
}

/// Truncate a string to at most n characters (UTF-8 safe)
fn truncate_str(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

/// Get human-readable language name
fn language_name(code: &str) -> &'static str {
    match code {
        "hi" | "hin" => "Hindi",
        "en" | "eng" => "English",
        "fr" | "fra" => "French",
        "es" | "spa" => "Spanish",
        "de" | "deu" => "German",
        "fil" | "tgl" => "Filipino",
        "zh" | "zho" => "Chinese",
        "ja" | "jpn" => "Japanese",
        "ko" | "kor" => "Korean",
        "ru" | "rus" => "Russian",
        "ar" | "ara" => "Arabic",
        "pt" | "por" => "Portuguese",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_name() {
        assert_eq!(language_name("hi"), "Hindi");
        assert_eq!(language_name("en"), "English");
        assert_eq!(language_name("unknown"), "Unknown");
    }
}
