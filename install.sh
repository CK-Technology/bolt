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

setup_nginx_config() {
    log "Setting up nginx configuration for bolt.cktech.org..."

    # Check if nginx is installed
    if ! command -v nginx &> /dev/null; then
        warn "nginx not found. Installing..."
        case $OS in
            ubuntu|debian)
                apt-get install -y nginx
                ;;
            fedora|centos|rhel)
                if command -v dnf &> /dev/null; then
                    dnf install -y nginx
                else
                    yum install -y nginx
                fi
                ;;
            arch)
                pacman -S --noconfirm nginx
                ;;
            opensuse|sles)
                zypper install -y nginx
                ;;
        esac
    fi

    # Create nginx site configuration
    cat > "/etc/nginx/sites-available/bolt.cktech.org" << 'EOF'
server {
    listen 80;
    server_name bolt.cktech.org;

    # Redirect all HTTP to HTTPS
    return 301 https://$host$request_uri;
}

server {
    listen 443 ssl http2;
    server_name bolt.cktech.org;

    # SSL certificates (will be configured by acme.sh)
    ssl_certificate /etc/nginx/ssl/bolt.cktech.org/fullchain.pem;
    ssl_certificate_key /etc/nginx/ssl/bolt.cktech.org/private.pem;

    # SSL configuration
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-RSA-AES128-GCM-SHA256:ECDHE-RSA-AES256-GCM-SHA384:ECDHE-RSA-AES128-SHA256:ECDHE-RSA-AES256-SHA384;
    ssl_prefer_server_ciphers on;
    ssl_session_cache shared:SSL:10m;
    ssl_session_timeout 10m;

    # Security headers
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
    add_header X-Frame-Options DENY;
    add_header X-Content-Type-Options nosniff;

    # Root directory for static files
    root /var/www/bolt.cktech.org;
    index index.html index.htm;

    # API proxy to Bolt daemon
    location /api/ {
        proxy_pass http://127.0.0.1:7878/;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # WebSocket support for real-time features
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }

    # Static files
    location / {
        try_files $uri $uri/ =404;
    }

    # Install script endpoint
    location /install.sh {
        add_header Content-Type "text/plain";
        alias /var/www/bolt.cktech.org/install.sh;
    }

    # Package repositories
    location /packages/ {
        autoindex on;
        alias /var/www/bolt.cktech.org/packages/;
    }
}
EOF

    # Create web directory
    mkdir -p /var/www/bolt.cktech.org/packages
    mkdir -p /etc/nginx/ssl/bolt.cktech.org

    # Enable site
    if [[ -d /etc/nginx/sites-enabled ]]; then
        ln -sf /etc/nginx/sites-available/bolt.cktech.org /etc/nginx/sites-enabled/
    fi

    log "✓ nginx configuration created"
}

install_acme_sh() {
    log "Installing acme.sh for SSL certificates..."

    if [[ ! -d ~/.acme.sh ]]; then
        curl https://get.acme.sh | sh
        source ~/.acme.sh/acme.sh.env
    fi

    # Issue certificate for bolt.cktech.org
    ~/.acme.sh/acme.sh --issue -d bolt.cktech.org --nginx

    # Install certificate
    ~/.acme.sh/acme.sh --install-cert -d bolt.cktech.org \
        --key-file /etc/nginx/ssl/bolt.cktech.org/private.pem \
        --fullchain-file /etc/nginx/ssl/bolt.cktech.org/fullchain.pem \
        --reloadcmd "systemctl reload nginx"

    log "✓ SSL certificate installed"
}

create_web_content() {
    log "Creating web content..."

    # Copy this install script to web directory
    cp "$0" /var/www/bolt.cktech.org/install.sh

    # Create index page
    cat > /var/www/bolt.cktech.org/index.html << 'EOF'
<!DOCTYPE html>
<html>
<head>
    <title>Bolt Container Runtime</title>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; line-height: 1.6; }
        .container { max-width: 800px; margin: 0 auto; }
        code { background: #f4f4f4; padding: 2px 4px; border-radius: 3px; }
        pre { background: #f4f4f4; padding: 10px; border-radius: 5px; overflow-x: auto; }
        .header { text-align: center; margin-bottom: 40px; }
        .section { margin: 30px 0; }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>Bolt Container Runtime</h1>
            <p>Next-generation Rust container runtime with gaming optimizations, GPU passthrough, and QUIC networking</p>
        </div>

        <div class="section">
            <h2>Quick Install</h2>
            <pre><code>curl -sSL https://bolt.cktech.org/install.sh | sudo bash</code></pre>
        </div>

        <div class="section">
            <h2>Manual Install</h2>
            <pre><code>wget https://bolt.cktech.org/install.sh
chmod +x install.sh
sudo ./install.sh</code></pre>
        </div>

        <div class="section">
            <h2>Package Repositories</h2>
            <p><a href="/packages/">Browse packages</a> for your distribution:</p>
            <ul>
                <li>Arch Linux: <code>/packages/arch/</code></li>
                <li>Debian/Ubuntu: <code>/packages/debian/</code></li>
            </ul>
        </div>

        <div class="section">
            <h2>Documentation</h2>
            <p>Visit our <a href="https://github.com/CK-Technology/bolt">GitHub repository</a> for documentation and examples.</p>
        </div>
    </div>
</body>
</html>
EOF

    log "✓ Web content created"
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

    # Web server setup (optional)
    if [[ "${SETUP_WEB:-}" == "true" ]]; then
        setup_nginx_config
        install_acme_sh
        create_web_content
        systemctl enable nginx
        systemctl restart nginx
    fi

    log "Installation complete!"
    log "Run 'sudo systemctl start bolt' to start the Bolt daemon"
    log "Run 'bolt --help' for usage information"

    if [[ "${SETUP_WEB:-}" == "true" ]]; then
        log "Web interface available at: https://bolt.cktech.org"
    fi
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --setup-web)
            SETUP_WEB=true
            shift
            ;;
        --help)
            echo "Bolt Container Runtime Installation Script"
            echo ""
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --setup-web    Setup nginx and SSL for bolt.cktech.org"
            echo "  --help         Show this help message"
            exit 0
            ;;
        *)
            error "Unknown option: $1"
            ;;
    esac
done

# Run main installation
main