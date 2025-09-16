# Maintainer: CK Technology <ghostkellz@proton.me>
pkgname=bolt
pkgver=0.1.0
pkgrel=1
pkgdesc="Next-generation Rust container runtime with gaming optimizations, GPU passthrough, and QUIC networking"
arch=('x86_64' 'aarch64')
url="https://github.com/CK-Technology/bolt"
license=('MIT')
depends=('glibc' 'gcc-libs' 'systemd' 'wayland' 'libxkbcommon' 'libxcb')
makedepends=('rust' 'cargo' 'git')
optdepends=(
    'nvidia-utils: NVIDIA GPU support'
    'vulkan-driver: Vulkan graphics support'
    'docker: Docker compatibility layer'
    'podman: Podman compatibility'
)
provides=('container-runtime')
conflicts=('bolt-git')
backup=(
    'etc/bolt/bolt.toml'
)
source=("git+https://github.com/CK-Technology/bolt.git#tag=v$pkgver")
sha256sums=('SKIP')

prepare() {
    cd "$pkgname"

    # Update Cargo.lock if needed
    cargo fetch --locked --target "$CARCH-unknown-linux-gnu"
}

build() {
    cd "$pkgname"

    export RUSTUP_TOOLCHAIN=stable
    export CARGO_TARGET_DIR=target

    # Build with all features enabled for Arch
    cargo build \
        --frozen \
        --release \
        --features "gaming,quic-networking,oci-runtime,nvidia-support" \
        --target "$CARCH-unknown-linux-gnu"
}

check() {
    cd "$pkgname"

    # Run tests (exclude integration tests that require root)
    cargo test \
        --frozen \
        --release \
        --features "gaming,quic-networking,oci-runtime,nvidia-support" \
        --target "$CARCH-unknown-linux-gnu" \
        --lib
}

package() {
    cd "$pkgname"

    # Install binary
    install -Dm755 "target/$CARCH-unknown-linux-gnu/release/bolt" \
        "$pkgdir/usr/bin/bolt"

    # Install systemd service
    install -Dm644 <(cat << 'EOF'
[Unit]
Description=Bolt Container Runtime
Documentation=https://github.com/CK-Technology/bolt
After=network.target
Wants=network.target

[Service]
Type=notify
ExecStart=/usr/bin/bolt daemon
ExecReload=/bin/kill -s HUP $MAINPID
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
) "$pkgdir/usr/lib/systemd/system/bolt.service"

    # Install default configuration
    install -Dm644 <(cat << 'EOF'
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
) "$pkgdir/etc/bolt/bolt.toml"

    # Install sysusers.d configuration
    install -Dm644 <(cat << 'EOF'
#Type Name ID GECOS Home directory Shell
u bolt - "Bolt Container Runtime" /var/lib/bolt /usr/bin/nologin
EOF
) "$pkgdir/usr/lib/sysusers.d/bolt.conf"

    # Install tmpfiles.d configuration
    install -Dm644 <(cat << 'EOF'
#Type Path Mode UID GID Age Argument
d /var/lib/bolt 0755 bolt bolt -
d /run/bolt 0755 bolt bolt -
d /tmp/bolt 0755 bolt bolt -
d /etc/bolt/certs 0700 bolt bolt -
f /var/log/bolt.log 0644 bolt bolt -
EOF
) "$pkgdir/usr/lib/tmpfiles.d/bolt.conf"

    # Install license
    install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"

    # Install documentation
    install -Dm644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"

    # Install man page (if exists)
    if [[ -f docs/bolt.1 ]]; then
        install -Dm644 docs/bolt.1 "$pkgdir/usr/share/man/man1/bolt.1"
    fi

    # Install bash completion (if exists)
    if [[ -f completions/bolt.bash ]]; then
        install -Dm644 completions/bolt.bash \
            "$pkgdir/usr/share/bash-completion/completions/bolt"
    fi

    # Install zsh completion (if exists)
    if [[ -f completions/_bolt ]]; then
        install -Dm644 completions/_bolt \
            "$pkgdir/usr/share/zsh/site-functions/_bolt"
    fi

    # Install fish completion (if exists)
    if [[ -f completions/bolt.fish ]]; then
        install -Dm644 completions/bolt.fish \
            "$pkgdir/usr/share/fish/vendor_completions.d/bolt.fish"
    fi
}

# Post-install message
post_install() {
    echo "Bolt container runtime has been installed."
    echo ""
    echo "To start using Bolt:"
    echo "  1. Enable and start the service:"
    echo "     sudo systemctl enable --now bolt"
    echo ""
    echo "  2. Add your user to the bolt group (optional, for rootless mode):"
    echo "     sudo usermod -aG bolt \$USER"
    echo ""
    echo "  3. Configure bolt in /etc/bolt/bolt.toml"
    echo ""
    echo "For GPU support, ensure you have the appropriate drivers installed:"
    echo "  - NVIDIA: nvidia-utils package"
    echo "  - AMD: mesa and vulkan-radeon packages"
    echo ""
    echo "Documentation: https://github.com/CK-Technology/bolt"
}

post_upgrade() {
    echo "Bolt has been upgraded to version $pkgver-$pkgrel"
    echo ""
    echo "Please restart the bolt service:"
    echo "  sudo systemctl restart bolt"
    echo ""
    echo "Check the changelog for any configuration changes:"
    echo "  https://github.com/CK-Technology/bolt/releases"
}

post_remove() {
    echo "Bolt has been removed."
    echo ""
    echo "To completely clean up:"
    echo "  sudo rm -rf /var/lib/bolt"
    echo "  sudo rm -rf /etc/bolt"
    echo "  sudo userdel bolt"
    echo ""
    echo "Note: Your container data may still be present in /var/lib/bolt"
}