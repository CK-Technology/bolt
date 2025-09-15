use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info};

/// S3-compatible object storage client for MinIO and Ghostbay integration
#[derive(Debug, Clone)]
pub struct S3StorageClient {
    pub endpoint: String,
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket: String,
    pub client: Option<aws_sdk_s3::Client>,
    pub provider: S3Provider,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum S3Provider {
    MinIO {
        endpoint: String,
        tls: bool,
    },
    Ghostbay {
        endpoint: String,
        cluster_id: Option<String>,
    },
    AWS {
        region: String,
    },
    Generic {
        endpoint: String,
        path_style: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3VolumeConfig {
    pub provider: S3Provider,
    pub bucket: String,
    pub prefix: Option<String>,
    pub access_key: String,
    pub secret_key: String,
    pub encryption: Option<S3Encryption>,
    pub compression: bool,
    pub cache_enabled: bool,
    pub cache_ttl_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Encryption {
    pub method: String, // AES256, aws:kms, etc.
    pub kms_key_id: Option<String>,
}

impl S3StorageClient {
    pub async fn new(config: S3VolumeConfig) -> Result<Self> {
        info!("🗄️  Initializing S3 storage client");

        let (endpoint, region) = match &config.provider {
            S3Provider::MinIO { endpoint, .. } => {
                info!("  🪣 Provider: MinIO at {}", endpoint);
                (endpoint.clone(), "us-east-1".to_string())
            }
            S3Provider::Ghostbay {
                endpoint,
                cluster_id,
            } => {
                info!("  👻 Provider: Ghostbay at {}", endpoint);
                if let Some(cluster) = cluster_id {
                    info!("    🏭 Cluster: {}", cluster);
                }
                (endpoint.clone(), "ghostbay-region".to_string())
            }
            S3Provider::AWS { region } => {
                info!("  ☁️  Provider: AWS S3 in {}", region);
                ("https://s3.amazonaws.com".to_string(), region.clone())
            }
            S3Provider::Generic {
                endpoint,
                path_style,
            } => {
                info!(
                    "  🔧 Provider: Generic S3 at {} (path-style: {})",
                    endpoint, path_style
                );
                (endpoint.clone(), "us-east-1".to_string())
            }
        };

        // Initialize AWS SDK config
        let aws_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .endpoint_url(&endpoint)
            .region(aws_config::Region::new(region.clone()))
            .credentials_provider(aws_sdk_s3::config::Credentials::new(
                &config.access_key,
                &config.secret_key,
                None,
                None,
                "bolt-runtime",
            ))
            .load()
            .await;

        let mut s3_config_builder = aws_sdk_s3::config::Builder::from(&aws_config);

        // Configure for non-AWS providers
        match &config.provider {
            S3Provider::MinIO { .. }
            | S3Provider::Ghostbay { .. }
            | S3Provider::Generic {
                path_style: true, ..
            } => {
                s3_config_builder = s3_config_builder.force_path_style(true);
            }
            _ => {}
        }

        let client = aws_sdk_s3::Client::from_conf(s3_config_builder.build());

        let storage_client = Self {
            endpoint,
            region,
            access_key: config.access_key,
            secret_key: config.secret_key,
            bucket: config.bucket.clone(),
            client: Some(client),
            provider: config.provider,
        };

        // Test connection and create bucket if needed
        storage_client.ensure_bucket_exists().await?;

        info!(
            "✅ S3 storage client initialized for bucket: {}",
            config.bucket
        );
        Ok(storage_client)
    }

    async fn ensure_bucket_exists(&self) -> Result<()> {
        let client = self.client.as_ref().unwrap();

        debug!("🔍 Checking if bucket '{}' exists", self.bucket);

        // Try to head the bucket first
        match client.head_bucket().bucket(&self.bucket).send().await {
            Ok(_) => {
                debug!("✅ Bucket '{}' exists", self.bucket);
                return Ok(());
            }
            Err(_) => {
                info!("📦 Bucket '{}' doesn't exist, creating...", self.bucket);
            }
        }

        // Create the bucket
        let mut create_request = client.create_bucket().bucket(&self.bucket);

        // Set location constraint for non-US regions
        if self.region != "us-east-1" {
            create_request = create_request.create_bucket_configuration(
                aws_sdk_s3::types::CreateBucketConfiguration::builder()
                    .location_constraint(aws_sdk_s3::types::BucketLocationConstraint::from(
                        self.region.as_str(),
                    ))
                    .build(),
            );
        }

        match create_request.send().await {
            Ok(_) => {
                info!("✅ Bucket '{}' created successfully", self.bucket);
            }
            Err(e) => {
                // Bucket might have been created by another process
                if e.to_string().contains("BucketAlreadyExists")
                    || e.to_string().contains("BucketAlreadyOwnedByYou")
                {
                    info!("✅ Bucket '{}' already exists", self.bucket);
                } else {
                    return Err(anyhow::anyhow!(
                        "Failed to create bucket '{}': {}",
                        self.bucket,
                        e
                    ));
                }
            }
        }

        Ok(())
    }

    pub async fn upload_file(&self, local_path: &Path, s3_key: &str) -> Result<()> {
        info!(
            "📤 Uploading {} to s3://{}/{}",
            local_path.display(),
            self.bucket,
            s3_key
        );

        let client = self.client.as_ref().unwrap();

        // Read file content
        let body = aws_smithy_types::byte_stream::ByteStream::from_path(local_path)
            .await
            .with_context(|| format!("Failed to read file: {:?}", local_path))?;

        // Upload to S3
        let mut put_request = client
            .put_object()
            .bucket(&self.bucket)
            .key(s3_key)
            .body(body);

        // Add metadata
        put_request = put_request
            .metadata("uploaded-by", "bolt-runtime")
            .metadata("upload-time", &chrono::Utc::now().to_rfc3339());

        // Handle provider-specific optimizations
        match &self.provider {
            S3Provider::Ghostbay { .. } => {
                // Ghostbay-specific optimizations
                put_request = put_request.metadata("ghostbay-optimized", "true");
                debug!("  👻 Applied Ghostbay optimizations");
            }
            S3Provider::MinIO { .. } => {
                // MinIO-specific optimizations
                put_request = put_request.metadata("minio-optimized", "true");
                debug!("  🪣 Applied MinIO optimizations");
            }
            _ => {}
        }

        put_request
            .send()
            .await
            .with_context(|| format!("Failed to upload {} to S3", s3_key))?;

        info!("✅ Upload complete: {}", s3_key);
        Ok(())
    }

    pub async fn download_file(&self, s3_key: &str, local_path: &Path) -> Result<()> {
        info!(
            "📥 Downloading s3://{}/{} to {}",
            self.bucket,
            s3_key,
            local_path.display()
        );

        let client = self.client.as_ref().unwrap();

        // Create parent directories
        if let Some(parent) = local_path.parent() {
            fs::create_dir_all(parent)
                .await
                .with_context(|| format!("Failed to create directory: {:?}", parent))?;
        }

        // Download from S3
        let response = client
            .get_object()
            .bucket(&self.bucket)
            .key(s3_key)
            .send()
            .await
            .with_context(|| format!("Failed to download {} from S3", s3_key))?;

        // Write to local file
        let body_bytes = response
            .body
            .collect()
            .await
            .with_context(|| "Failed to read S3 response body")?
            .into_bytes();

        fs::write(local_path, body_bytes)
            .await
            .with_context(|| format!("Failed to write file: {:?}", local_path))?;

        info!("✅ Download complete: {}", local_path.display());
        Ok(())
    }

    pub async fn list_objects(&self, prefix: Option<&str>) -> Result<Vec<S3ObjectInfo>> {
        debug!("📋 Listing objects in bucket: {}", self.bucket);

        let client = self.client.as_ref().unwrap();

        let mut list_request = client.list_objects_v2().bucket(&self.bucket);

        if let Some(prefix) = prefix {
            list_request = list_request.prefix(prefix);
            debug!("  🔍 Using prefix: {}", prefix);
        }

        let response = list_request
            .send()
            .await
            .with_context(|| "Failed to list S3 objects")?;

        let mut objects = Vec::new();

        if let Some(contents) = response.contents {
            for object in contents {
                if let (Some(key), Some(size), Some(last_modified)) =
                    (object.key, object.size, object.last_modified)
                {
                    objects.push(S3ObjectInfo {
                        key,
                        size: size as u64,
                        last_modified: last_modified.secs() as u64,
                        etag: object.e_tag,
                        storage_class: object.storage_class.map(|sc| sc.as_str().to_string()),
                    });
                }
            }
        }

        debug!("📋 Found {} objects", objects.len());
        Ok(objects)
    }

    pub async fn delete_object(&self, s3_key: &str) -> Result<()> {
        info!("🗑️  Deleting s3://{}/{}", self.bucket, s3_key);

        let client = self.client.as_ref().unwrap();

        client
            .delete_object()
            .bucket(&self.bucket)
            .key(s3_key)
            .send()
            .await
            .with_context(|| format!("Failed to delete {} from S3", s3_key))?;

        info!("✅ Object deleted: {}", s3_key);
        Ok(())
    }

    pub async fn copy_object(&self, source_key: &str, dest_key: &str) -> Result<()> {
        info!(
            "📋 Copying s3://{}/{} to s3://{}/{}",
            self.bucket, source_key, self.bucket, dest_key
        );

        let client = self.client.as_ref().unwrap();

        let copy_source = format!("{}/{}", self.bucket, source_key);

        client
            .copy_object()
            .bucket(&self.bucket)
            .key(dest_key)
            .copy_source(&copy_source)
            .send()
            .await
            .with_context(|| format!("Failed to copy {} to {}", source_key, dest_key))?;

        info!("✅ Object copied: {} -> {}", source_key, dest_key);
        Ok(())
    }

    pub async fn get_object_metadata(&self, s3_key: &str) -> Result<HashMap<String, String>> {
        debug!("📄 Getting metadata for s3://{}/{}", self.bucket, s3_key);

        let client = self.client.as_ref().unwrap();

        let response = client
            .head_object()
            .bucket(&self.bucket)
            .key(s3_key)
            .send()
            .await
            .with_context(|| format!("Failed to get metadata for {}", s3_key))?;

        let mut metadata = HashMap::new();

        if let Some(meta) = response.metadata {
            for (key, value) in meta {
                metadata.insert(key, value);
            }
        }

        // Add standard metadata
        if let Some(size) = response.content_length {
            metadata.insert("content-length".to_string(), size.to_string());
        }

        if let Some(content_type) = response.content_type {
            metadata.insert("content-type".to_string(), content_type);
        }

        if let Some(etag) = response.e_tag {
            metadata.insert("etag".to_string(), etag);
        }

        Ok(metadata)
    }

    /// Create a presigned URL for temporary access
    pub async fn create_presigned_url(
        &self,
        s3_key: &str,
        expires_in_seconds: u64,
    ) -> Result<String> {
        info!(
            "🔗 Creating presigned URL for s3://{}/{} (expires in {}s)",
            self.bucket, s3_key, expires_in_seconds
        );

        let client = self.client.as_ref().unwrap();

        let presigning_config = aws_sdk_s3::presigning::PresigningConfig::expires_in(
            std::time::Duration::from_secs(expires_in_seconds),
        )
        .with_context(|| "Failed to create presigning config")?;

        let presigned_request = client
            .get_object()
            .bucket(&self.bucket)
            .key(s3_key)
            .presigned(presigning_config)
            .await
            .with_context(|| "Failed to create presigned URL")?;

        let url = presigned_request.uri().to_string();
        info!("✅ Presigned URL created: {}", url);

        Ok(url)
    }

    /// Specialized method for Ghostbay integration
    pub async fn ghostbay_cluster_status(&self) -> Result<GhostbayClusterInfo> {
        if !matches!(self.provider, S3Provider::Ghostbay { .. }) {
            return Err(anyhow::anyhow!(
                "Ghostbay cluster status only available for Ghostbay provider"
            ));
        }

        info!("👻 Getting Ghostbay cluster status");

        // Ghostbay-specific API calls would go here
        // For now, we'll return a mock response
        Ok(GhostbayClusterInfo {
            cluster_id: "ghostbay-cluster-1".to_string(),
            status: "healthy".to_string(),
            nodes: 3,
            total_storage: 1_000_000_000_000, // 1TB
            used_storage: 250_000_000_000,    // 250GB
            version: "0.1.0".to_string(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3ObjectInfo {
    pub key: String,
    pub size: u64,
    pub last_modified: u64,
    pub etag: Option<String>,
    pub storage_class: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostbayClusterInfo {
    pub cluster_id: String,
    pub status: String,
    pub nodes: u32,
    pub total_storage: u64,
    pub used_storage: u64,
    pub version: String,
}

/// S3 volume driver for Bolt
pub struct S3VolumeDriver {
    pub client: S3StorageClient,
    pub local_cache_dir: PathBuf,
    pub cache_enabled: bool,
}

impl S3VolumeDriver {
    pub async fn new(config: S3VolumeConfig, cache_dir: PathBuf) -> Result<Self> {
        info!("🗄️  Initializing S3 volume driver");

        let client = S3StorageClient::new(config.clone()).await?;

        // Create cache directory
        if config.cache_enabled {
            fs::create_dir_all(&cache_dir)
                .await
                .with_context(|| format!("Failed to create cache directory: {:?}", cache_dir))?;
            info!("💾 Local cache enabled at: {:?}", cache_dir);
        }

        Ok(Self {
            client,
            local_cache_dir: cache_dir,
            cache_enabled: config.cache_enabled,
        })
    }

    pub async fn mount_volume(&self, volume_name: &str, mount_point: &Path) -> Result<()> {
        info!(
            "🔗 Mounting S3 volume '{}' at {:?}",
            volume_name, mount_point
        );

        // Create mount point
        fs::create_dir_all(mount_point)
            .await
            .with_context(|| format!("Failed to create mount point: {:?}", mount_point))?;

        if self.cache_enabled {
            // Set up cache-backed mounting
            let cache_path = self.local_cache_dir.join(volume_name);
            fs::create_dir_all(&cache_path).await?;

            // Sync from S3 to cache
            self.sync_from_s3(&format!("volumes/{}/", volume_name), &cache_path)
                .await?;

            info!("✅ S3 volume mounted with cache: {}", volume_name);
        } else {
            // Direct S3 access (would need FUSE or similar for real implementation)
            info!("✅ S3 volume mounted (direct access): {}", volume_name);
        }

        Ok(())
    }

    pub async fn unmount_volume(&self, volume_name: &str) -> Result<()> {
        info!("🔌 Unmounting S3 volume: {}", volume_name);

        if self.cache_enabled {
            // Sync cache back to S3
            let cache_path = self.local_cache_dir.join(volume_name);
            self.sync_to_s3(&cache_path, &format!("volumes/{}/", volume_name))
                .await?;
        }

        info!("✅ S3 volume unmounted: {}", volume_name);
        Ok(())
    }

    async fn sync_from_s3(&self, s3_prefix: &str, local_dir: &Path) -> Result<()> {
        debug!("⬇️  Syncing from S3: {} -> {:?}", s3_prefix, local_dir);

        let objects = self.client.list_objects(Some(s3_prefix)).await?;

        for object in objects {
            let relative_path = object.key.strip_prefix(s3_prefix).unwrap_or(&object.key);
            let local_file = local_dir.join(relative_path);

            if let Some(parent) = local_file.parent() {
                fs::create_dir_all(parent).await?;
            }

            self.client.download_file(&object.key, &local_file).await?;
        }

        debug!("✅ Sync from S3 complete");
        Ok(())
    }

    async fn sync_to_s3(&self, local_dir: &Path, s3_prefix: &str) -> Result<()> {
        debug!("⬆️  Syncing to S3: {:?} -> {}", local_dir, s3_prefix);

        let mut dir = fs::read_dir(local_dir).await?;

        while let Some(entry) = dir.next_entry().await? {
            let local_path = entry.path();
            let relative_path = local_path.strip_prefix(local_dir)?;
            let s3_key = format!("{}{}", s3_prefix, relative_path.to_string_lossy());

            if local_path.is_file() {
                self.client.upload_file(&local_path, &s3_key).await?;
            } else if local_path.is_dir() {
                // Recursively sync directories using Box::pin for indirection
                Box::pin(self.sync_to_s3(&local_path, &format!("{}/", s3_key))).await?;
            }
        }

        debug!("✅ Sync to S3 complete");
        Ok(())
    }
}
