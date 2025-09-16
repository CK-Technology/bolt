#!/bin/bash
set -euo pipefail

# Package build script for Bolt
# Builds both Arch and Debian packages

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_DIR="$SCRIPT_DIR/build"
PACKAGE_DIR="$SCRIPT_DIR/packages"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log() {
    echo -e "${GREEN}[BUILD]${NC} $1"
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
        OS_ID=$ID
        OS_VERSION=$VERSION_ID
    else
        error "Cannot detect OS"
    fi
}

check_dependencies() {
    log "Checking build dependencies..."

    # Common dependencies
    local deps=("git" "curl" "wget")

    case $OS_ID in
        arch|manjaro)
            deps+=("base-devel" "rust" "cargo")
            ;;
        ubuntu|debian)
            deps+=("build-essential" "devscripts" "debhelper" "cargo" "rustc")
            ;;
        fedora|centos|rhel)
            deps+=("rpm-build" "rpmdevtools" "cargo" "rust")
            ;;
    esac

    for dep in "${deps[@]}"; do
        if ! command -v "$dep" &> /dev/null && ! rpm -q "$dep" &> /dev/null && ! dpkg -l "$dep" &> /dev/null && ! pacman -Q "$dep" &> /dev/null; then
            warn "Missing dependency: $dep"
        fi
    done

    log "✓ Dependencies checked"
}

build_arch_package() {
    log "Building Arch Linux package..."

    cd "$SCRIPT_DIR"

    # Clean previous builds
    rm -rf pkg src *.pkg.tar.* || true

    # Build package
    makepkg -sf --noconfirm

    # Move packages to output directory
    mkdir -p "$PACKAGE_DIR/arch"
    mv *.pkg.tar.* "$PACKAGE_DIR/arch/" || true

    log "✓ Arch package built successfully"
}

build_debian_package() {
    log "Building Debian package..."

    cd "$SCRIPT_DIR"

    # Clean previous builds
    rm -rf debian/.debhelper debian/bolt debian/files debian/*.log || true

    # Build package
    dpkg-buildpackage -us -uc -b

    # Move packages to output directory
    mkdir -p "$PACKAGE_DIR/debian"
    mv ../*.deb "$PACKAGE_DIR/debian/" 2>/dev/null || true
    mv ../*.changes "$PACKAGE_DIR/debian/" 2>/dev/null || true
    mv ../*.buildinfo "$PACKAGE_DIR/debian/" 2>/dev/null || true

    log "✓ Debian package built successfully"
}

build_rpm_package() {
    log "Building RPM package..."

    # Create RPM build environment
    rpmdev-setuptree

    # Create spec file
    cat > ~/rpmbuild/SPECS/bolt.spec << 'EOF'
Name:           bolt
Version:        0.1.0
Release:        1%{?dist}
Summary:        Next-generation Rust container runtime with gaming optimizations

License:        MIT
URL:            https://github.com/CK-Technology/bolt
Source0:        https://github.com/CK-Technology/bolt/archive/v%{version}.tar.gz

BuildRequires:  rust >= 1.85
BuildRequires:  cargo
BuildRequires:  systemd-devel
BuildRequires:  wayland-devel
BuildRequires:  libxkbcommon-devel
BuildRequires:  libxcb-devel

Requires:       systemd
Requires:       wayland
Requires:       libxkbcommon
Requires:       libxcb

%description
Bolt is a next-generation container runtime written in Rust that provides
gaming optimizations, GPU passthrough support, QUIC networking, and native
Wayland support for GUI applications.

%prep
%autosetup

%build
cargo build --release --features "gaming,quic-networking,oci-runtime,nvidia-support"

%install
install -D -m755 target/release/bolt %{buildroot}%{_bindir}/bolt

# Install systemd service
install -D -m644 debian/bolt.service %{buildroot}%{_unitdir}/bolt.service

# Install configuration
install -D -m644 debian/bolt.toml %{buildroot}%{_sysconfdir}/bolt/bolt.toml

# Create directories
mkdir -p %{buildroot}%{_sharedstatedir}/bolt
mkdir -p %{buildroot}%{_rundir}/bolt

%pre
getent group bolt >/dev/null || groupadd -r bolt
getent passwd bolt >/dev/null || useradd -r -g bolt -d %{_sharedstatedir}/bolt -s /sbin/nologin bolt

%post
%systemd_post bolt.service

%preun
%systemd_preun bolt.service

%postun
%systemd_postun_with_restart bolt.service

%files
%license LICENSE
%doc README.md
%{_bindir}/bolt
%{_unitdir}/bolt.service
%config(noreplace) %{_sysconfdir}/bolt/bolt.toml
%attr(755,bolt,bolt) %dir %{_sharedstatedir}/bolt
%attr(755,bolt,bolt) %dir %{_rundir}/bolt

%changelog
* Mon Sep 16 2024 CK Technology <ghostkellz@proton.me> - 0.1.0-1
- Initial RPM release
EOF

    # Build RPM
    rpmbuild -ba ~/rpmbuild/SPECS/bolt.spec

    # Move packages to output directory
    mkdir -p "$PACKAGE_DIR/rpm"
    cp ~/rpmbuild/RPMS/*/*.rpm "$PACKAGE_DIR/rpm/" || true
    cp ~/rpmbuild/SRPMS/*.rpm "$PACKAGE_DIR/rpm/" || true

    log "✓ RPM package built successfully"
}

create_repository_metadata() {
    log "Creating repository metadata..."

    # Arch repository
    if [[ -d "$PACKAGE_DIR/arch" ]] && [[ -n "$(ls -A "$PACKAGE_DIR/arch"/*.pkg.tar.* 2>/dev/null)" ]]; then
        cd "$PACKAGE_DIR/arch"
        repo-add bolt.db.tar.gz *.pkg.tar.*
        log "✓ Arch repository created"
    fi

    # Debian repository
    if [[ -d "$PACKAGE_DIR/debian" ]] && [[ -n "$(ls -A "$PACKAGE_DIR/debian"/*.deb 2>/dev/null)" ]]; then
        cd "$PACKAGE_DIR/debian"

        # Create Packages file
        dpkg-scanpackages . /dev/null | gzip -9c > Packages.gz

        # Create Release file
        cat > Release << EOF
Origin: CK Technology
Label: Bolt Container Runtime
Suite: stable
Codename: stable
Architectures: amd64 arm64
Components: main
Description: Bolt container runtime packages
EOF

        log "✓ Debian repository created"
    fi

    # RPM repository
    if [[ -d "$PACKAGE_DIR/rpm" ]] && [[ -n "$(ls -A "$PACKAGE_DIR/rpm"/*.rpm 2>/dev/null)" ]]; then
        cd "$PACKAGE_DIR/rpm"
        createrepo .
        log "✓ RPM repository created"
    fi
}

copy_to_web() {
    if [[ -n "${WEB_DIR:-}" ]] && [[ -d "$WEB_DIR" ]]; then
        log "Copying packages to web directory..."
        cp -r "$PACKAGE_DIR"/* "$WEB_DIR/packages/"
        log "✓ Packages copied to $WEB_DIR"
    fi
}

main() {
    log "Starting package build process..."

    detect_os
    check_dependencies

    # Clean and create output directories
    rm -rf "$PACKAGE_DIR"
    mkdir -p "$PACKAGE_DIR"

    # Build packages based on current OS
    case $OS_ID in
        arch|manjaro)
            build_arch_package
            ;;
        ubuntu|debian)
            build_debian_package
            ;;
        fedora|centos|rhel)
            build_rpm_package
            ;;
        *)
            warn "Unsupported OS: $OS_ID. Building with install script only."
            ;;
    esac

    create_repository_metadata
    copy_to_web

    log "Package build complete!"
    log "Packages available in: $PACKAGE_DIR"

    if [[ -d "$PACKAGE_DIR" ]]; then
        log "Built packages:"
        find "$PACKAGE_DIR" -name "*.pkg.tar.*" -o -name "*.deb" -o -name "*.rpm" | while read -r pkg; do
            info "  $(basename "$pkg")"
        done
    fi
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --web-dir)
            WEB_DIR="$2"
            shift 2
            ;;
        --arch-only)
            BUILD_ARCH_ONLY=true
            shift
            ;;
        --debian-only)
            BUILD_DEBIAN_ONLY=true
            shift
            ;;
        --rpm-only)
            BUILD_RPM_ONLY=true
            shift
            ;;
        --help)
            echo "Bolt Package Build Script"
            echo ""
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --web-dir DIR      Copy packages to web directory"
            echo "  --arch-only        Build only Arch packages"
            echo "  --debian-only      Build only Debian packages"
            echo "  --rpm-only         Build only RPM packages"
            echo "  --help             Show this help message"
            exit 0
            ;;
        *)
            error "Unknown option: $1"
            ;;
    esac
done

# Run main build process
main