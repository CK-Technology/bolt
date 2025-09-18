use thiserror::Error;

/// Bolt-specific error types for better error handling
#[derive(Error, Debug)]
pub enum BoltError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("Container runtime error: {0}")]
    Runtime(#[from] RuntimeError),

    #[error("Network error: {0}")]
    Network(#[from] NetworkError),

    #[error("Gaming setup error: {0}")]
    Gaming(#[from] GamingError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML serialization error: {0}")]
    Serialization(#[from] toml::de::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Generic error: {0}")]
    Other(#[from] anyhow::Error),
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Boltfile not found at path: {path}")]
    BoltfileNotFound { path: String },

    #[error("Invalid Boltfile format: {reason}")]
    InvalidFormat { reason: String },

    #[error("Missing required field: {field}")]
    MissingField { field: String },
}

#[derive(Error, Debug)]
pub enum RuntimeError {
    #[error("Container not found: {name}")]
    ContainerNotFound { name: String },

    #[error("Image pull failed: {image}")]
    ImagePullFailed { image: String },

    #[error("Container start failed: {reason}")]
    StartFailed { reason: String },

    #[error("OCI runtime error: {message}")]
    OciError { message: String },
}

#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("Network not found: {name}")]
    NetworkNotFound { name: String },

    #[error("Invalid subnet: {subnet}")]
    InvalidSubnet { subnet: String },

    #[error("QUIC setup failed: {reason}")]
    QuicSetupFailed { reason: String },
}

#[derive(Error, Debug)]
pub enum GamingError {
    #[error("GPU not found or not supported")]
    GpuNotFound,

    #[error("Wine/Proton setup failed: {reason}")]
    WineSetupFailed { reason: String },

    #[error("Audio system not available: {system}")]
    AudioUnavailable { system: String },

    #[error("Gaming optimization failed: {reason}")]
    OptimizationFailed { reason: String },
}

/// Convenience type alias for Bolt results
pub type Result<T, E = BoltError> = std::result::Result<T, E>;
