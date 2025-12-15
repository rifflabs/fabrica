//! Configuration management for Palace Fabrica

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

/// Main configuration structure
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub discord: DiscordConfig,
    pub database: DatabaseConfig,
    pub translation: TranslationConfig,
    pub plane: PlaneConfig,
    pub github: GithubConfig,
    pub webhooks: WebhookConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiscordConfig {
    pub token: String,
    pub application_id: u64,
    #[serde(default)]
    pub guild_id: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_db_path")]
    pub path: String,
}

fn default_db_path() -> String {
    "fabrica.db".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct TranslationConfig {
    #[serde(default = "default_backend")]
    pub backend: String,
    #[serde(default = "default_palace_url")]
    pub palace_url: String,
    #[serde(default = "default_openrouter_url")]
    pub openrouter_url: String,
    #[serde(default = "default_openrouter_api_key")]
    pub openrouter_api_key: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_language")]
    pub default_language: String,
    #[serde(default = "default_languages")]
    pub supported_languages: Vec<String>,
}

fn default_backend() -> String {
    "palace".to_string()
}

fn default_palace_url() -> String {
    "http://localhost:19848".to_string()
}

fn default_openrouter_url() -> String {
    "https://openrouter.ai/api/v1".to_string()
}

fn default_openrouter_api_key() -> String {
    "".to_string()
}

fn default_model() -> String {
    "mistral".to_string()
}

fn default_language() -> String {
    "en".to_string()
}

fn default_languages() -> Vec<String> {
    vec!["en".to_string(), "hi".to_string()]
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlaneConfig {
    pub url: String,
    pub api_key: String,
    #[serde(default = "default_workspace")]
    pub workspace: String,
}

fn default_workspace() -> String {
    "riff".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct GithubConfig {
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub webhook_secret: Option<String>,
    #[serde(default)]
    pub org: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WebhookConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub base_url: Option<String>,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8080
}

impl Config {
    /// Load configuration from fabrica.toml
    pub fn load() -> Result<Self> {
        Self::load_from("fabrica.toml")
    }

    /// Load configuration from a specific path
    pub fn load_from<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();

        // Try to load from file first
        if path.exists() {
            let content = std::fs::read_to_string(path)
                .with_context(|| format!("Failed to read config from {}", path.display()))?;

            let mut config: Config = toml::from_str(&content)
                .with_context(|| format!("Failed to parse config from {}", path.display()))?;

            // Expand environment variables
            config.expand_env_vars();
            return Ok(config);
        }

        // Fall back to environment variables only
        Self::from_env()
    }

    /// Load configuration entirely from environment variables
    pub fn from_env() -> Result<Self> {
        Ok(Config {
            discord: DiscordConfig {
                token: std::env::var("DISCORD_TOKEN")
                    .context("DISCORD_TOKEN environment variable required")?,
                application_id: std::env::var("DISCORD_APP_ID")
                    .unwrap_or_else(|_| "0".to_string())
                    .parse()
                    .unwrap_or(0),
                guild_id: std::env::var("DISCORD_GUILD_ID")
                    .ok()
                    .and_then(|s| s.parse().ok()),
            },
            database: DatabaseConfig {
                path: std::env::var("DATABASE_PATH").unwrap_or_else(|_| default_db_path()),
            },
            translation: TranslationConfig {
                backend: std::env::var("TRANSLATION_BACKEND").unwrap_or_else(|_| default_backend()),
                palace_url: std::env::var("PALACE_URL").unwrap_or_else(|_| default_palace_url()),
                openrouter_url: std::env::var("OPENROUTER_URL").unwrap_or_else(|_| default_openrouter_url()),
                openrouter_api_key: std::env::var("OPENROUTER_API_KEY").unwrap_or_else(|_| default_openrouter_api_key()),
                model: std::env::var("TRANSLATION_MODEL").unwrap_or_else(|_| default_model()),
                default_language: default_language(),
                supported_languages: default_languages(),
            },
            plane: PlaneConfig {
                url: std::env::var("PLANE_URL").unwrap_or_else(|_| "https://plane.riff.cc".to_string()),
                api_key: std::env::var("PLANE_API_KEY").unwrap_or_default(),
                workspace: std::env::var("PLANE_WORKSPACE").unwrap_or_else(|_| default_workspace()),
            },
            github: GithubConfig {
                token: std::env::var("GITHUB_TOKEN").ok(),
                webhook_secret: std::env::var("GITHUB_WEBHOOK_SECRET").ok(),
                org: std::env::var("GITHUB_ORG").ok(),
            },
            webhooks: WebhookConfig {
                host: std::env::var("WEBHOOK_HOST").unwrap_or_else(|_| default_host()),
                port: std::env::var("WEBHOOK_PORT")
                    .unwrap_or_else(|_| default_port().to_string())
                    .parse()
                    .unwrap_or(default_port()),
                base_url: std::env::var("WEBHOOK_BASE_URL").ok(),
            },
        })
    }

    /// Expand ${VAR} patterns in string fields
    fn expand_env_vars(&mut self) {
        self.discord.token = expand_env(&self.discord.token);
        self.plane.api_key = expand_env(&self.plane.api_key);
        if let Some(ref mut token) = self.github.token {
            *token = expand_env(token);
        }
        if let Some(ref mut secret) = self.github.webhook_secret {
            *secret = expand_env(secret);
        }
    }
}

/// Expand ${VAR} patterns in a string
fn expand_env(s: &str) -> String {
    let mut result = s.to_string();

    // Find all ${VAR} patterns
    while let Some(start) = result.find("${") {
        if let Some(end) = result[start..].find('}') {
            let var_name = &result[start + 2..start + end];
            let replacement = std::env::var(var_name).unwrap_or_default();
            result = format!("{}{}{}", &result[..start], replacement, &result[start + end + 1..]);
        } else {
            break;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_env() {
        std::env::set_var("TEST_VAR", "hello");
        assert_eq!(expand_env("${TEST_VAR}"), "hello");
        assert_eq!(expand_env("prefix_${TEST_VAR}_suffix"), "prefix_hello_suffix");
        std::env::remove_var("TEST_VAR");
    }
}
