use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::ProfileRepository;
use crate::config::UserConfig;
use crate::optimizations::OptimizationProfile;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileSubmission {
    pub profile: OptimizationProfile,
    pub author_email: String,
    pub description: String,
    pub tags: Vec<String>,
    pub compatible_games: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatingSubmission {
    pub profile_name: String,
    pub rating: f32,
    pub user_id: String,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileReport {
    pub profile_name: String,
    pub reason: String,
    pub details: String,
    pub reporter_id: String,
}

pub async fn submit_profile(
    profile: &OptimizationProfile,
    repository: &ProfileRepository,
) -> Result<()> {
    let client = reqwest::Client::new();
    let user_config = UserConfig::load().unwrap_or_default();

    // Auto-generate tags based on profile characteristics
    let mut tags = vec!["gaming".to_string()];

    // Add performance-related tags
    if profile.name.to_lowercase().contains("performance") {
        tags.push("high-performance".to_string());
    }
    if profile.name.to_lowercase().contains("latency")
        || profile.name.to_lowercase().contains("low-latency")
    {
        tags.push("low-latency".to_string());
    }

    // Add platform tags
    if profile.name.to_lowercase().contains("amd")
        || profile.description.to_lowercase().contains("amd")
    {
        tags.push("amd".to_string());
    }
    if profile.name.to_lowercase().contains("nvidia")
        || profile.description.to_lowercase().contains("nvidia")
    {
        tags.push("nvidia".to_string());
    }

    // Auto-detect compatible games from profile name/description
    let mut compatible_games = Vec::new();
    let game_keywords = [
        "counter-strike",
        "cs2",
        "dota",
        "apex",
        "valorant",
        "fortnite",
        "overwatch",
        "warzone",
        "minecraft",
        "wow",
    ];

    for keyword in &game_keywords {
        if profile.name.to_lowercase().contains(keyword)
            || profile.description.to_lowercase().contains(keyword)
        {
            compatible_games.push(keyword.to_string());
            tags.push(format!("game-{}", keyword));
        }
    }

    let submission = ProfileSubmission {
        profile: profile.clone(),
        author_email: user_config.get_user_email_or_default(),
        description: profile.description.clone(),
        tags,
        compatible_games,
    };

    let response = client
        .post(format!("{}/submit", repository.url))
        .json(&submission)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to submit profile: {}",
            response.status()
        ));
    }

    Ok(())
}

pub async fn submit_rating(profile_name: &str, rating: f32) -> Result<()> {
    let client = reqwest::Client::new();
    let user_config = UserConfig::load().unwrap_or_default();

    let rating_submission = RatingSubmission {
        profile_name: profile_name.to_string(),
        rating,
        user_id: user_config.get_user_id_or_anonymous(),
        comment: None,
    };

    let response = client
        .post("https://community.bolt.dev/profiles/rate")
        .json(&rating_submission)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to submit rating: {}",
            response.status()
        ));
    }

    Ok(())
}

pub async fn report_profile(profile_name: &str, reason: &str) -> Result<()> {
    let client = reqwest::Client::new();
    let user_config = UserConfig::load().unwrap_or_default();

    let report = ProfileReport {
        profile_name: profile_name.to_string(),
        reason: reason.to_string(),
        details: "User report".to_string(),
        reporter_id: user_config.get_user_id_or_anonymous(),
    };

    let response = client
        .post("https://community.bolt.dev/profiles/report")
        .json(&report)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to report profile: {}",
            response.status()
        ));
    }

    Ok(())
}
