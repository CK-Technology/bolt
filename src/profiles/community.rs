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

    let submission = ProfileSubmission {
        profile: profile.clone(),
        author_email: user_config.get_user_email_or_default(),
        description: profile.description.clone(),
        tags: vec!["gaming".to_string()], // TODO: Auto-generate tags
        compatible_games: vec![],         // TODO: Auto-detect compatible games
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
