//! Translation service - Routes to Palace Translator or other backends
//!
//! Uses cheap LLMs (Mistral, Devstral) for translation to minimize costs.

use crate::config::TranslationConfig;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

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
    pub async fn translate(&self, text: &str, from: &str, to: &str) -> Result<String> {
        match self.config.backend.as_str() {
            "openrouter" => self.translate_via_openrouter(text, from, to).await,
            "direct" => self.translate_direct(text, from, to).await,
            other => {
                warn!("Unknown translation backend: {}, falling back to direct", other);
                self.translate_direct(text, from, to).await
            }
        }
    }

    /// Translate using OpenRouter API
    async fn translate_via_openrouter(&self, text: &str, from: &str, to: &str) -> Result<String> {
        let from_name = language_name(from);
        let to_name = language_name(to);

        let prompt = format!(
            "Translate the following {} text to {}. \
             Output ONLY the translation, nothing else. \
             Do not add explanations or notes.\n\n{}",
            from_name, to_name, text
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
            .json(&request)
            .send()
            .await
            .context("Failed to send OpenRouter translation request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
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

        Ok(translation)
    }

    /// Direct translation using simple word substitution (fallback)
    async fn translate_direct(&self, text: &str, _from: &str, _to: &str) -> Result<String> {
        // This is a placeholder - in production, you'd use a proper translation API
        // For now, just return the original text with a note
        warn!("Direct translation not implemented, returning original");
        Ok(format!("[Translation unavailable] {}", text))
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
