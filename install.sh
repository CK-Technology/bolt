#!/bin/bash
set -euo pipefail

# Bolt Container Runtime Installation Script
# Supports Linux distributions with automatic detection

BOLT_VERSION="0.1.0"
BOLT_REPO="https://github.com/CK-Technology/bolt"
INSTALL_DIR="/usr/local/bin"
SERVICE_DIR="/etc/systemd/system"
CONFIG_DIR="/etc/bolt"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log() {
    echo -e "${GREEN}[BOLT]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
    exit 1
}

info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

detect_os() {
    if [[ -f /etc/os-release ]]; then
        . /etc/os-release
        OS=$ID
        VERSION=$VERSION_ID
    else
        error "Cannot detect OS. /etc/os-release not found."
    fi
}

check_requirements() {
    log "Checking system requirements..."

    # Check if running as root
    if [[ $EUID -ne 0 ]]; then
        error "This script must be run as root (use sudo)"
    fi

    # Check system architecture
    ARCH=$(uname -m)
    case $ARCH in
        x86_64|amd64)
            ARCH="x86_64"
            ;;
        aarch64|arm64)
            ARCH="aarch64"
            ;;
        *)
            error "Unsupported architecture: $ARCH"
            ;;
    esac

    # Check kernel version for container support
    KERNEL_VERSION=$(uname -r | cut -d. -f1-2)
    if ! awk "BEGIN {exit !($KERNEL_VERSION >= 4.4)}"; then
        error "Kernel version $KERNEL_VERSION is too old. Required: 4.4+"
    fi

    log "✓ System requirements met"
}

install_dependencies() {
    log "Installing system dependencies..."

    case $OS in
        ubuntu|debian)
            apt-get update
            apt-get install -y \
                curl \
                wget \
                ca-certificates \
                gnupg \
                lsb-release \
                systemd \
                libwayland-dev \
                libxkbcommon-dev \
                libxcb1-dev
            ;;
        fedora|centos|rhel)
            if command -v dnf &> /dev/null; then
                dnf install -y \
                    curl \
                    wget \
                    ca-certificates \
                    systemd \
                    wayland-devel \
                    libxkbcommon-devel \
                    libxcb-devel
            else
                yum install -y \
                    curl \
                    wget \
                    ca-certificates \
                    systemd \
                    wayland-devel \
                    libxkbcommon-devel \
                    libxcb-devel
            fi
            ;;
        arch)
            pacman -Syu --noconfirm \
                curl \
                wget \
                ca-certificates \
                systemd \
                wayland \
                libxkbcommon \
                libxcb
            ;;
        opensuse|sles)
            zypper install -y \
                curl \
                wget \
                ca-certificates \
                systemd \
                wayland-devel \
                libxkbcommon-devel \
                libxcb-devel
            ;;
        *)
            warn "Unknown OS: $OS. Please install dependencies manually."
            ;;
    esac

    log "✓ Dependencies installed"
}

install_rust() {
    log "Checking Rust installation..."

    if ! command -v rustc &> /dev/null; then
        log "Installing Rust..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source ~/.cargo/env
        rustup default stable
    else
        log "✓ Rust already installed"
    fi

    # Ensure minimum Rust version
    RUST_VERSION=$(rustc --version | cut -d' ' -f2)
    if ! awk "BEGIN {exit !(\"$RUST_VERSION\" >= \"1.85\")}"; then
        log "Updating Rust to minimum version 1.85..."
        rustup update stable
    fi
}

download_and_build() {
    log "Downloading and building Bolt..."

    TEMP_DIR=$(mktemp -d)
    cd "$TEMP_DIR"

    # Clone repository
    git clone "$BOLT_REPO" bolt
    cd bolt

    # Build with optimizations
    export CARGO_TARGET_DIR="$TEMP_DIR/target"
    cargo build --release --features "gaming,quic-networking,oci-runtime,nvidia-support"

    # Install binary
    install -m 755 "$TEMP_DIR/target/release/bolt" "$INSTALL_DIR/bolt"

    log "✓ Bolt binary installed to $INSTALL_DIR/bolt"

    # Cleanup
    cd /
    rm -rf "$TEMP_DIR"
}

create_config() {
    log "Creating configuration files..."

    mkdir -p "$CONFIG_DIR"

    cat > "$CONFIG_DIR/bolt.toml" << 'EOF'
# Bolt Container Runtime Configuration

[runtime]
# Container runtime settings
root = "/var/lib/bolt"
state = "/run/bolt"
temp = "/tmp/bolt"

[networking]
# QUIC networking configuration
enable_quic = true
bind_address = "0.0.0.0:7878"
certificate_path = "/etc/bolt/certs"

[gaming]
# Gaming optimizations
gpu_passthrough = true
wayland_support = true
audio_passthrough = true

[security]
# Security settings
enable_seccomp = true
enable_selinux = true
rootless_mode = false

[logging]
level = "info"
file = "/var/log/bolt.log"
EOF

    # Create directories
    mkdir -p /var/lib/bolt
    mkdir -p /run/bolt
    mkdir -p /tmp/bolt
    mkdir -p /var/log
    mkdir -p /etc/bolt/certs

    log "✓ Configuration created"
}

create_systemd_service() {
    log "Creating systemd service..."

    cat > "$SERVICE_DIR/bolt.service" << EOF
[Unit]
Description=Bolt Container Runtime
Documentation=https://github.com/CK-Technology/bolt
After=network.target
Wants=network.target

[Service]
Type=notify
ExecStart=$INSTALL_DIR/bolt daemon
ExecReload=/bin/kill -s HUP \$MAINPID
TimeoutStopSec=0
KillMode=process
Restart=on-failure
RestartSec=5
Delegate=yes
LimitNOFILE=1048576
LimitNPROC=1048576
LimitCORE=infinity
TasksMax=infinity

[Install]
WantedBy=multi-user.target
EOF

    systemctl daemon-reload
    systemctl enable bolt.service

    log "✓ Systemd service created and enabled"
}


main() {
    log "Starting Bolt installation..."

    detect_os
    check_requirements
    install_dependencies
    install_rust
    download_and_build
    create_config
    create_systemd_service

    log "Installation complete!"
    log "Run 'sudo systemctl start bolt' to start the Bolt daemon"
    log "Run 'bolt --help' for usage information"
}

# Run main installation
main