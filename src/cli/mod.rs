use clap::{Parser, Subcommand};

pub mod compat;

#[derive(Parser)]
#[command(name = "bolt")]
#[command(about = "Next-generation container runtime for Linux gaming and development")]
#[command(version, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,

    /// Configuration file path
    #[arg(short, long, default_value = "Boltfile.toml")]
    pub config: String,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run a single container/capsule
    Run {
        /// Image or capsule to run
        image: String,

        /// Container name
        #[arg(short, long)]
        name: Option<String>,

        /// Port mappings (host:container)
        #[arg(short, long)]
        ports: Vec<String>,

        /// Environment variables
        #[arg(short, long)]
        env: Vec<String>,

        /// Volume mounts (host:container)
        #[arg(short, long)]
        volumes: Vec<String>,

        /// Run in detached mode
        #[arg(short, long)]
        detach: bool,
    },

    /// Build a container image
    Build {
        /// Path to build context
        #[arg(default_value = ".")]
        path: String,

        /// Image tag
        #[arg(short, long)]
        tag: Option<String>,

        /// Dockerfile path
        #[arg(short, long, default_value = "Dockerfile")]
        file: String,
    },

    /// Pull an image from registry
    Pull {
        /// Image name
        image: String,
    },

    /// Push an image to registry
    Push {
        /// Image name
        image: String,
    },

    /// List containers
    Ps {
        /// Show all containers (including stopped)
        #[arg(short, long)]
        all: bool,
    },

    /// Stop containers
    Stop {
        /// Container names or IDs
        containers: Vec<String>,
    },

    /// Remove containers
    Rm {
        /// Container names or IDs
        containers: Vec<String>,

        /// Force removal
        #[arg(short, long)]
        force: bool,
    },

    /// Surge orchestration commands (like docker-compose)
    Surge {
        #[command(subcommand)]
        command: SurgeCommands,
    },

    /// Gaming-specific commands
    Gaming {
        #[command(subcommand)]
        command: GamingCommands,
    },

    /// Network management
    Network {
        #[command(subcommand)]
        command: NetworkCommands,
    },

    /// Docker/Podman compatibility layer
    Compat {
        #[command(subcommand)]
        command: compat::CompatCommands,
    },
}

#[derive(Subcommand)]
pub enum SurgeCommands {
    /// Start services from Boltfile
    Up {
        /// Services to start (default: all)
        services: Vec<String>,

        /// Detached mode
        #[arg(short, long)]
        detach: bool,

        /// Recreate containers
        #[arg(long)]
        force_recreate: bool,
    },

    /// Stop services
    Down {
        /// Services to stop (default: all)
        services: Vec<String>,

        /// Remove volumes
        #[arg(short, long)]
        volumes: bool,
    },

    /// Show service status
    Status,

    /// Show service logs
    Logs {
        /// Service name
        service: Option<String>,

        /// Follow logs
        #[arg(short, long)]
        follow: bool,

        /// Number of lines to show
        #[arg(short, long)]
        tail: Option<usize>,
    },

    /// Scale services
    Scale {
        /// Service scaling (service=count)
        services: Vec<String>,
    },
}

#[derive(Subcommand)]
pub enum GamingCommands {
    /// Configure GPU passthrough
    Gpu {
        #[command(subcommand)]
        command: GpuCommands,
    },

    /// Setup Wine/Proton container
    Wine {
        /// Proton version
        #[arg(long)]
        proton: Option<String>,

        /// Windows version to emulate
        #[arg(long)]
        winver: Option<String>,
    },

    /// Configure audio for gaming
    Audio {
        /// Audio system (pipewire, pulseaudio)
        #[arg(long, default_value = "pipewire")]
        system: String,
    },

    /// Launch a game
    Launch {
        /// Game executable or script
        game: String,

        /// Launch arguments
        args: Vec<String>,
    },

    /// Start Wayland gaming session
    Wayland,

    /// Configure real-time gaming optimizations
    Realtime {
        /// Enable optimizations
        #[arg(long)]
        enable: bool,
    },

    /// Optimize a running game process
    Optimize {
        /// Process ID of the game
        pid: u32,
    },

    /// Show gaming performance report
    Performance,
}

#[derive(Subcommand)]
pub enum GpuCommands {
    /// List available GPUs
    List,

    /// Configure NVIDIA GPU
    Nvidia {
        /// GPU device index
        #[arg(long)]
        device: Option<u32>,

        /// Enable DLSS
        #[arg(long)]
        dlss: bool,

        /// Enable ray tracing
        #[arg(long)]
        raytracing: bool,
    },

    /// Configure AMD GPU
    Amd {
        /// GPU device index
        #[arg(long)]
        device: Option<u32>,
    },
}

#[derive(Subcommand)]
pub enum NetworkCommands {
    /// Create network
    Create {
        /// Network name
        name: String,

        /// Network driver
        #[arg(long, default_value = "bolt")]
        driver: String,

        /// Subnet CIDR
        #[arg(long)]
        subnet: Option<String>,
    },

    /// List networks
    List,

    /// Remove network
    Remove {
        /// Network name
        name: String,
    },
}
