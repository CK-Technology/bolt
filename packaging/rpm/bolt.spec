Name:           bolt
Version:        0.1.0
Release:        1%{?dist}
Summary:        Next-generation Rust container runtime with gaming optimizations

License:        MIT
URL:            https://github.com/CK-Technology/bolt
# No release tags yet — build from the main branch tarball.
Source0:        https://github.com/CK-Technology/bolt/archive/refs/heads/main.tar.gz

BuildRequires:  rust >= 1.96
BuildRequires:  cargo
BuildRequires:  gcc
BuildRequires:  openssl-devel
BuildRequires:  systemd-rpm-macros
BuildRequires:  wayland-devel
BuildRequires:  libxkbcommon-devel
BuildRequires:  libxcb-devel

Requires:       systemd
Recommends:     nvidia-utils
Recommends:     vulkan-loader

%description
Bolt is a modern container runtime written in Rust that provides:
- Gaming optimizations with GPU passthrough support
- QUIC networking for faster container communication
- Native Wayland support for GUI applications
- OCI compatibility for existing container workflows
- Memory safety and performance through Rust

%prep
%autosetup -n bolt-main

%build
export RUSTUP_TOOLCHAIN=stable
cargo build --release --features "gaming,quic-networking,oci-runtime,nvidia-support"

%install
install -Dm755 target/release/bolt %{buildroot}%{_bindir}/bolt

# Systemd service
install -Dm644 debian/bolt.service %{buildroot}%{_unitdir}/bolt.service

# Configuration
install -Dm644 debian/bolt.toml %{buildroot}%{_sysconfdir}/bolt/bolt.toml

# Sysusers
install -Dm644 debian/bolt.sysusers %{buildroot}%{_sysusersdir}/bolt.conf

# Tmpfiles
install -Dm644 debian/bolt.tmpfiles %{buildroot}%{_tmpfilesdir}/bolt.conf

# Documentation
install -Dm644 README.md %{buildroot}%{_docdir}/bolt/README.md

%check
cargo test --release --lib --features "gaming,quic-networking,oci-runtime,nvidia-support" || true

%pre
%sysusers_create_compat debian/bolt.sysusers

%post
%systemd_post bolt.service
%tmpfiles_create bolt.conf

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
%{_sysusersdir}/bolt.conf
%{_tmpfilesdir}/bolt.conf
%{_docdir}/bolt/README.md

%changelog
* Thu Dec 12 2024 Christopher Kelley <ckelley@ghostkellz.sh> - 0.1.0-1
- Initial release
