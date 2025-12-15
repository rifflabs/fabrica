//! Palace Fabrica - Coordination infrastructure for Riff Labs
//!
//! This crate provides both the Discord bot binary and reusable library components
//! for translation, status tracking, and project integration.

pub mod bot;
pub mod config;
pub mod db;
pub mod modules;
pub mod services;
pub mod webhooks;

pub use config::Config;
pub use db::Database;
