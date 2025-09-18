use anyhow::Result;
use std::collections::HashMap;

use super::{ProfileEntry, ProfileMetadata, ProfileRepository, ProfileSource};
use crate::optimizations::OptimizationProfile;

pub async fn fetch_profiles(
    repository: &ProfileRepository,
) -> Result<HashMap<String, ProfileEntry>> {
    let client = reqwest::Client::new();
    let url = format!("{}/profiles", repository.url);

    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to fetch profiles from {}: {}",
            repository.name,
            response.status()
        ));
    }

    let profiles: HashMap<String, serde_json::Value> = response.json().await?;
    let mut result = HashMap::new();

    for (name, profile_data) in profiles {
        if let Ok(profile) =
            serde_json::from_value::<OptimizationProfile>(profile_data["profile"].clone())
        {
            if let Ok(metadata) =
                serde_json::from_value::<ProfileMetadata>(profile_data["metadata"].clone())
            {
                let entry = ProfileEntry {
                    profile,
                    metadata,
                    source: ProfileSource::Community {
                        repository: repository.name.clone(),
                    },
                };
                result.insert(name, entry);
            }
        }
    }

    Ok(result)
}
