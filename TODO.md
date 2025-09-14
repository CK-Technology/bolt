Why Rust (for Bolt/Surge)

Proven in this exact domain

OCI runtime: youki (Rust) is real and fast; study/borrow pieces or interop with it.

OCI image tooling: oci-spec-rs, containerd-client, oras-rs, tar, zstd, async-compression.

Kernel plumbing: nix, caps, libseccomp-sys, cgroups-rs (v2), mount, rust-iptables / nftnl.

Networking stack you want

QUIC: quinn (mature), TLS: rustls. Great fit for your QUIC fabric vision.

Rootless net: slirp4netns bindings exist; or drive pasta/vpnkit style helpers.

Wayland/gaming UX

Wayland: wayland-client, smithay-client-toolkit.

UI: egui/eframe for a native GUI with no npm; winit for windowing; integrates cleanly with gaming workflows.

GPU inside containers: NVIDIA (nvidia-container-toolkit + libnvidia-container), AMD (rocm paths); Rust is friendlier to wrap/drive these CLIs + configs.

Security model

Memory safety, rich seccomp + landlock bindings, capability dropping, serde + toml for strict config parsing.

Dev velocity

Async runtime (tokio), superb testing, fuzzing (cargo-fuzz), and cross-compilation with cross.

Zig today lacks ready-made crates for OCI + kernel plumbing. You’ll spend time re-implementing what Rust already has. If you really want Zig, use it later for hot paths as a small FFI’d lib.

Architecture suggestion (Rust)
