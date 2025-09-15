use super::*;
use crate::config::{Auth, BoltFile, Service, Storage};
use crate::error::{BoltError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Docker Compose file structure for parsing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeFile {
    pub version: Option<String>,
    pub services: HashMap<String, ComposeService>,
    pub networks: Option<HashMap<String, ComposeNetwork>>,
    pub volumes: Option<HashMap<String, ComposeVolume>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeService {
    pub image: Option<String>,
    pub build: Option<ComposeBuild>,
    pub ports: Option<Vec<String>>,
    pub volumes: Option<Vec<String>>,
    pub environment: Option<HashMap<String, String>>,
    pub depends_on: Option<Vec<String>>,
    pub networks: Option<Vec<String>>,
    pub restart: Option<String>,
    pub command: Option<String>,
    pub entrypoint: Option<String>,
    pub working_dir: Option<String>,
    pub user: Option<String>,
    pub privileged: Option<bool>,
    pub mem_limit: Option<String>,
    pub cpus: Option<String>,
    pub labels: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ComposeBuild {
    Simple(String),
    Complex {
        context: String,
        dockerfile: Option<String>,
        args: Option<HashMap<String, String>>,
        target: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeNetwork {
    pub driver: Option<String>,
    pub ipam: Option<ComposeIpam>,
    pub external: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeIpam {
    pub config: Option<Vec<ComposeIpamConfig>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeIpamConfig {
    pub subnet: Option<String>,
    pub gateway: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeVolume {
    pub driver: Option<String>,
    pub external: Option<bool>,
}

pub struct ComposeCompat;

impl ComposeCompat {
    /// Convert Docker Compose file to Boltfile
    pub fn convert_compose_file(compose_content: &str) -> Result<String> {
        let compose: ComposeFile = serde_yaml::from_str(compose_content).map_err(|e| {
            BoltError::Config(crate::error::ConfigError::InvalidFormat {
                reason: format!("Failed to parse compose file: {}", e),
            })
        })?;

        let mut boltfile = BoltFile {
            project: "converted-from-compose".to_string(),
            services: HashMap::new(),
            networks: None,
        };

        // Convert services
        for (name, compose_service) in compose.services {
            let bolt_service = Self::convert_service(&compose_service)?;
            boltfile.services.insert(name, bolt_service);
        }

        // Convert to TOML
        let toml_content = toml::to_string_pretty(&boltfile)
            .map_err(|e| BoltError::Config(format!("Failed to generate Boltfile: {}", e)))?;

        Ok(format!(
            "# Converted from Docker Compose\n# Date: {}\n# Note: Review and adjust as needed\n\n{}",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
            toml_content
        ))
    }

    /// Convert individual compose service to Bolt service
    fn convert_service(compose_service: &ComposeService) -> Result<Service> {
        let mut service = Service::default();

        // Basic image/build configuration
        if let Some(image) = &compose_service.image {
            service.image = Some(image.clone());
        }

        if let Some(build) = &compose_service.build {
            match build {
                ComposeBuild::Simple(context) => {
                    service.build = Some(context.clone());
                }
                ComposeBuild::Complex { context, .. } => {
                    service.build = Some(context.clone());
                }
            }
        }

        // Port mappings
        if let Some(ports) = &compose_service.ports {
            service.ports = Some(ports.clone());
        }

        // Volume mappings
        if let Some(volumes) = &compose_service.volumes {
            service.volumes = Some(volumes.clone());
        }

        // Environment variables
        if let Some(env) = &compose_service.environment {
            service.env = Some(env.clone());
        }

        // Dependencies
        if let Some(depends_on) = &compose_service.depends_on {
            service.depends_on = Some(depends_on.clone());
        }

        // Network configuration
        if let Some(networks) = &compose_service.networks {
            if !networks.is_empty() {
                service.network = Some(networks[0].clone());
            }
        }

        // Resource limits and configuration
        if compose_service.privileged.unwrap_or(false)
            || compose_service.mem_limit.is_some()
            || compose_service.cpus.is_some()
            || compose_service.user.is_some()
        {
            // For complex configurations, suggest using capsules
            if compose_service.image.as_ref().map_or(false, |img| {
                img.contains("postgres")
                    || img.contains("mysql")
                    || img.contains("redis")
                    || img.contains("mongo")
            }) {
                // Convert database services to capsules
                if let Some(image) = &compose_service.image {
                    if image.contains("postgres") {
                        service.capsule = Some("postgres".to_string());
                        service.image = None; // Clear image when using capsule

                        // Set up database auth if environment variables suggest it
                        if let Some(env) = &compose_service.environment {
                            let user = env
                                .get("POSTGRES_USER")
                                .or_else(|| env.get("POSTGRES_DB"))
                                .unwrap_or("postgres");
                            let password = env.get("POSTGRES_PASSWORD").map_or("password", |v| v);

                            service.auth = Some(Auth {
                                user: user.to_string(),
                                password: password.to_string(),
                            });
                        }

                        // Set up storage if volumes are specified
                        if compose_service.volumes.is_some() {
                            service.storage = Some(Storage {
                                size: "10Gi".to_string(),
                                driver: None,
                            });
                        }
                    } else if image.contains("mysql") {
                        service.capsule = Some("mysql".to_string());
                        service.image = None;

                        if let Some(env) = &compose_service.environment {
                            let user = env.get("MYSQL_USER").map_or("mysql", |v| v);
                            let password = env
                                .get("MYSQL_PASSWORD")
                                .or_else(|| env.get("MYSQL_ROOT_PASSWORD"))
                                .map_or("password", |v| v);

                            service.auth = Some(Auth {
                                user: user.to_string(),
                                password: password.to_string(),
                            });
                        }

                        if compose_service.volumes.is_some() {
                            service.storage = Some(Storage {
                                size: "10Gi".to_string(),
                                driver: None,
                            });
                        }
                    } else if image.contains("redis") {
                        service.capsule = Some("redis".to_string());
                        service.image = None;
                    }
                }
            }
        }

        // Add conversion notes as comments (would be in TOML comments)
        let mut notes = Vec::new();

        if compose_service.privileged.unwrap_or(false) {
            notes.push("# Note: Privileged mode converted to capsule security");
        }

        if compose_service.mem_limit.is_some() || compose_service.cpus.is_some() {
            notes.push("# Note: Resource limits can be set in capsule configuration");
        }

        if compose_service.restart.is_some() {
            notes.push("# Note: Restart policies handled by Surge orchestration");
        }

        Ok(service)
    }

    /// Generate migration notes and recommendations
    pub fn generate_migration_notes(compose_content: &str) -> Result<String> {
        let compose: ComposeFile = serde_yaml::from_str(compose_content).map_err(|e| {
            BoltError::Config(crate::error::ConfigError::InvalidFormat {
                reason: format!("Failed to parse compose file: {}", e),
            })
        })?;

        let mut notes = Vec::new();
        notes.push("# Docker Compose to Bolt Migration Notes".to_string());
        notes.push("".to_string());

        // Analyze services
        notes.push("## Service Analysis:".to_string());
        for (name, service) in &compose.services {
            notes.push(format!("### Service: {}", name));

            if service.image.is_some() && service.build.is_some() {
                notes.push(
                    "  ⚠️  Both image and build specified - Bolt will prioritize build".to_string(),
                );
            }

            if let Some(image) = &service.image {
                if image.contains("postgres") || image.contains("mysql") || image.contains("redis")
                {
                    notes.push(format!(
                        "  ✨ Recommended: Use Bolt capsule for {} (better isolation)",
                        image
                    ));
                }
            }

            if service.privileged.unwrap_or(false) {
                notes.push(
                    "  🔒 Privileged mode detected - Consider capsule security model".to_string(),
                );
            }

            if let Some(networks) = &service.networks {
                if networks.len() > 1 {
                    notes.push("  🌐 Multiple networks detected - Bolt supports single network per service".to_string());
                }
            }

            notes.push("".to_string());
        }

        // Network analysis
        if let Some(networks) = &compose.networks {
            notes.push("## Network Analysis:".to_string());
            for (name, network) in networks {
                notes.push(format!("### Network: {}", name));

                if network.external.unwrap_or(false) {
                    notes.push("  📡 External network - Ensure it exists in Bolt".to_string());
                }

                if let Some(driver) = &network.driver {
                    match driver.as_str() {
                        "bridge" => notes.push("  ✅ Bridge driver supported".to_string()),
                        "overlay" => notes.push(
                            "  🚀 Consider Bolt's QUIC networking for overlay functionality"
                                .to_string(),
                        ),
                        _ => notes.push(format!(
                            "  ⚠️  Driver '{}' - Check Bolt compatibility",
                            driver
                        )),
                    }
                }
                notes.push("".to_string());
            }
        }

        // Volume analysis
        if let Some(volumes) = &compose.volumes {
            notes.push("## Volume Analysis:".to_string());
            for (name, volume) in volumes {
                notes.push(format!("### Volume: {}", name));

                if volume.external.unwrap_or(false) {
                    notes.push("  📦 External volume - Ensure it exists in Bolt".to_string());
                }

                if let Some(driver) = &volume.driver {
                    match driver.as_str() {
                        "local" => notes.push("  ✅ Local driver supported".to_string()),
                        _ => notes.push(format!(
                            "  💾 Consider Bolt's S3 or GhostBay storage for '{}' driver",
                            driver
                        )),
                    }
                }
                notes.push("".to_string());
            }
        }

        // Recommendations
        notes.push("## Migration Recommendations:".to_string());
        notes.push("".to_string());
        notes.push(
            "1. **Review Generated Boltfile**: Verify all services are correctly converted"
                .to_string(),
        );
        notes.push(
            "2. **Test Incrementally**: Start with individual services before full stack"
                .to_string(),
        );
        notes.push("3. **Leverage Bolt Features**:".to_string());
        notes.push("   - Use capsules for databases and stateful services".to_string());
        notes.push("   - Consider QUIC networking for improved performance".to_string());
        notes.push("   - Utilize gaming optimizations if applicable".to_string());
        notes.push(
            "4. **Update CI/CD**: Replace `docker-compose` commands with `bolt surge`".to_string(),
        );
        notes.push(
            "5. **Monitor Performance**: Bolt's async runtime may improve performance".to_string(),
        );
        notes.push("".to_string());
        notes.push("## Command Mapping:".to_string());
        notes.push("```bash".to_string());
        notes.push("# Docker Compose -> Bolt Surge".to_string());
        notes.push("docker-compose up      -> bolt surge up".to_string());
        notes.push("docker-compose down    -> bolt surge down".to_string());
        notes.push("docker-compose ps      -> bolt surge status".to_string());
        notes.push("docker-compose logs    -> bolt logs (coming soon)".to_string());
        notes.push("docker-compose exec    -> bolt exec (coming soon)".to_string());
        notes.push("```".to_string());

        Ok(notes.join("\n"))
    }

    /// Validate compose file for potential conversion issues
    pub fn validate_compose_file(compose_content: &str) -> Result<Vec<String>> {
        let compose: ComposeFile = serde_yaml::from_str(compose_content).map_err(|e| {
            BoltError::Config(crate::error::ConfigError::InvalidFormat {
                reason: format!("Failed to parse compose file: {}", e),
            })
        })?;

        let mut warnings = Vec::new();

        // Check version compatibility
        if let Some(version) = &compose.version {
            let version_num: f32 = version.parse().unwrap_or(2.0);
            if version_num < 2.0 {
                warnings.push(format!(
                    "Compose version {} is quite old, consider updating",
                    version
                ));
            } else if version_num > 3.8 {
                warnings.push(format!(
                    "Compose version {} is very recent, some features may not convert",
                    version
                ));
            }
        }

        // Check for unsupported features
        for (name, service) in &compose.services {
            if let Some(build) = &service.build {
                if let ComposeBuild::Complex {
                    args: Some(args), ..
                } = build
                {
                    if !args.is_empty() {
                        warnings.push(format!(
                            "Service '{}': Build args may need manual conversion",
                            name
                        ));
                    }
                }
            }

            if service.privileged.unwrap_or(false) {
                warnings.push(format!(
                    "Service '{}': Privileged mode - review security implications",
                    name
                ));
            }

            if let Some(networks) = &service.networks {
                if networks.len() > 1 {
                    warnings.push(format!(
                        "Service '{}': Multiple networks - Bolt supports single network",
                        name
                    ));
                }
            }
        }

        Ok(warnings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compose_conversion() {
        let compose_yaml = r#"
version: '3.8'
services:
  web:
    image: nginx:latest
    ports:
      - "80:80"
    depends_on:
      - api

  api:
    build: ./api
    ports:
      - "3000:3000"
    environment:
      - DATABASE_URL=postgresql://user:pass@db:5432/app
    depends_on:
      - db

  db:
    image: postgres:13
    environment:
      POSTGRES_DB: app
      POSTGRES_USER: user
      POSTGRES_PASSWORD: pass
    volumes:
      - postgres_data:/var/lib/postgresql/data

volumes:
  postgres_data:
"#;

        let result = ComposeCompat::convert_compose_file(compose_yaml);
        assert!(result.is_ok());

        let boltfile = result.unwrap();
        assert!(boltfile.contains("project = \"converted-from-compose\""));
        assert!(boltfile.contains("[services.web]"));
        assert!(boltfile.contains("[services.api]"));
        assert!(boltfile.contains("[services.db]"));
    }

    #[test]
    fn test_migration_notes() {
        let compose_yaml = r#"
version: '3.8'
services:
  web:
    image: nginx:latest
    privileged: true
"#;

        let notes = ComposeCompat::generate_migration_notes(compose_yaml).unwrap();
        assert!(notes.contains("Privileged mode"));
        assert!(notes.contains("Migration Recommendations"));
    }
}
