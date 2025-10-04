# 🎮 Gaming with Bolt

Bolt provides the high-performance container runtime and GPU passthrough for gaming. For the complete gaming experience, see [GhostForge](https://github.com/yourusername/ghostforge).

---

## Architecture

**Bolt** provides:
- Sub-microsecond GPU passthrough (130x faster than Docker)
- Container runtime and process isolation
- Wine/Proton container management
- Real-time CPU scheduling
- Snapshot/restore for game states

**GhostForge** provides:
- Gaming UI and game library
- Per-game profiles and optimizations
- Performance overlays
- Auto-tuning and benchmarks

---

## GPU Passthrough Performance

Bolt achieves **0.8μs GPU passthrough latency** vs Docker's 104μs:

```bash
# Run GPU benchmark
bolt gaming performance

# Output:
# GPU Passthrough Latency: 0.8μs
# Frame Time Consistency: 99.2% within 1ms
# Input Lag: <1ms additional overhead
```

---

## Quick Start

### 1. Verify GPU

```bash
bolt gpu list

# Output:
# ╔═══════╦══════════════════════╦════════════╗
# ║  ID   ║       Name          ║  Memory    ║
# ╠═══════╬══════════════════════╬════════════╣
# ║ gpu:0 ║ NVIDIA RTX 4090     ║ 24576 MB   ║
# ╚═══════╩══════════════════════╩════════════╝
```

### 2. Setup Gaming Container

```bash
# Create Wine gaming container with GPU
bolt gaming wine --proton 8.0

# Or use pre-configured profiles
bolt gaming optimize --pid <game_process_id>
```

### 3. Launch Game

```bash
# Launch Windows game via Wine/Proton
bolt gaming launch game.exe

# With specific GPU
bolt gaming launch --gpus device=0 game.exe
```

---

## Wine/Proton Containers

### Setup Wine Environment

```bash
# Setup Proton container (Steam compatibility)
bolt gaming wine --proton 8.0 --winver win11

# Setup standard Wine
bolt gaming wine --winver win10

# Launch game in container
bolt run --gpus 1 \
  -v ~/.local/share/Steam:/steam \
  -v ~/Games:/games \
  --name gaming-wine \
  bolt/wine-proton:8.0 \
  wine game.exe
```

### Audio Configuration

```bash
# Configure PipeWire (recommended)
bolt gaming audio --system pipewire

# Or PulseAudio
bolt gaming audio --system pulseaudio

# Run game with audio passthrough
bolt run --gpus 1 \
  -v /run/user/1000/pipewire-0:/run/pipewire \
  --device /dev/snd \
  bolt/wine-proton:8.0 \
  wine game.exe
```

---

## Wayland Gaming

For native Wayland support with GPU acceleration:

```bash
# Start Wayland gaming session
bolt gaming wayland

# Launches containerized Wayland session with:
# - Full GPU passthrough
# - Input device access
# - Display/monitor access
# - Audio routing

# Run game in Wayland container
bolt run --gpus 1 \
  -e WAYLAND_DISPLAY=wayland-0 \
  -v /run/user/1000/wayland-0:/run/wayland-0 \
  --device /dev/dri \
  --device /dev/input \
  native-game
```

---

## Real-Time Optimizations

### CPU Scheduling

```bash
# Enable real-time gaming optimizations
bolt gaming realtime --enable

# This configures:
# - CPU governor to 'performance'
# - Real-time process priority
# - CPU affinity for performance cores
# - Disables CPU frequency scaling

# Disable when done
bolt gaming realtime --disable
```

### Game Process Optimization

```bash
# Auto-optimize running game
bolt gaming optimize --pid 12345

# Manual optimizations:
# - Set CPU affinity to performance cores
# - Increase process priority
# - Pin memory pages (reduce latency)
# - Disable power management
```

---

## Gaming Profiles

Bolt includes pre-configured profiles for popular games:

```bash
# List available profiles
bolt gaming profiles list

# Output:
# Cyberpunk 2077      - RTX game, DLSS Quality, RT Psycho
# Counter-Strike 2    - Competitive FPS, low latency
# Elden Ring          - Medium settings, 60 FPS target
# Baldur's Gate 3     - High settings, DLSS Balanced
# Starfield           - High settings, DLSS Performance
# Hogwarts Legacy     - RT Medium, DLSS Quality
# Red Dead Redemption 2 - High/Ultra mixed
# The Witcher 3       - RT Ultra, DLSS Quality

# Apply profile
bolt gaming launch --profile "Cyberpunk 2077" game.exe
```

### Profile Details

Profiles configure:
- NVIDIA settings (DLSS, ray tracing, shader cache)
- Wine/Proton settings (DXVK, VKD3D)
- CPU affinity and governor
- Performance hints

---

## NVIDIA Features

### DLSS Support

```bash
# Enable DLSS for game
bolt gaming gpu nvidia --dlss --device 0

# DLSS modes:
# - Quality: Best image quality
# - Balanced: Balance quality/performance
# - Performance: Maximum FPS
# - Ultra Performance: Maximum FPS (4K only)
```

### Ray Tracing

```bash
# Enable ray tracing
bolt gaming gpu nvidia --raytracing --device 0

# Ray tracing quality presets:
# - Ultra: Maximum quality
# - High: Balanced
# - Medium: Performance-focused
# - Psycho: Cyberpunk 2077 mode
```

### AMD GPU

```bash
# Configure AMD GPU for gaming
bolt gaming gpu amd --device 0

# Enables:
# - FSR (FidelityFX Super Resolution)
# - Radeon Anti-Lag
# - Radeon Boost
# - Enhanced Sync
```

---

## Snapshots for Gaming

Save and restore complete game states:

```bash
# Create snapshot before game session
bolt snapshot create --with-gpu-state --name "before-raid"

# Play game...

# Restore to exact state (including GPU clocks, temps)
bolt snapshot rollback before-raid

# Use cases:
# - Save before difficult boss fights
# - Preserve working game configurations
# - Test different graphics settings
# - Quick save/load for speedruns
```

---

## Multi-GPU Gaming

### SLI/CrossFire Alternative

```bash
# Allocate multiple GPUs to game
bolt run --gpus 2 \
  -e CUDA_VISIBLE_DEVICES=0,1 \
  bolt/wine-proton:8.0 \
  wine game.exe

# Bolt schedules GPU workload efficiently
# Better than traditional SLI/CrossFire
```

### Mixed GPU Workloads

```bash
# Game on GPU 0, stream encoding on GPU 1
bolt run --gpus device=0 --name game wine game.exe
bolt run --gpus device=1 --name stream obs-studio

# Monitor GPU usage
bolt gpu status

# Output:
# ╔═══════╦════════════╦══════════════════════╗
# ║  GPU  ║ Container  ║     Utilization      ║
# ╠═══════╬════════════╬══════════════════════╣
# ║ gpu:0 ║ game       ║ ████████████░░ 98%   ║
# ║ gpu:1 ║ stream     ║ ████░░░░░░░░░░ 35%   ║
# ╚═══════╩════════════╩══════════════════════╝
```

---

## Performance Monitoring

### Real-Time Metrics

```bash
# Monitor GPU during gaming
bolt gpu metrics --container game

# Output (updates in real-time):
# GPU 0: RTX 4090
# ├─ Utilization: 98%
# ├─ Memory: 18234/24576 MB (74%)
# ├─ Temperature: 68°C
# ├─ Power: 380W / 450W
# ├─ Clock: 2580 MHz
# └─ FPS: 144 (estimated from frame time)
```

### Performance Report

```bash
# Generate gaming performance report
bolt gaming performance

# Output:
# ╔══════════════════════════════════════════════╗
# ║         Gaming Performance Report            ║
# ╠══════════════════════════════════════════════╣
# ║ GPU Passthrough Latency:     0.8μs           ║
# ║ Average Frame Time:           6.9ms (145FPS) ║
# ║ Frame Time Variance:          0.3ms          ║
# ║ 99th Percentile:              8.2ms          ║
# ║ Input Lag (additional):       0.7ms          ║
# ║ CPU Overhead:                 2.1%           ║
# ╚══════════════════════════════════════════════╝
```

---

## Example: Complete Gaming Setup

### Cyberpunk 2077

```bash
# 1. Setup environment
bolt gaming wine --proton 8.0 --winver win11
bolt gaming audio --system pipewire
bolt gaming realtime --enable

# 2. Configure NVIDIA
bolt gaming gpu nvidia --dlss --raytracing --device 0

# 3. Create game container
bolt run --gpus 1 \
  --name cyberpunk \
  -v ~/Games/Cyberpunk:/game \
  -v ~/.steam:/steam \
  -v /run/user/1000/pipewire-0:/run/pipewire \
  --device /dev/dri \
  --device /dev/input \
  --device /dev/snd \
  bolt/wine-proton:8.0 \
  wine /game/bin/x64/Cyberpunk2077.exe

# 4. Monitor performance
bolt gpu metrics --container cyberpunk

# 5. Create snapshot for save state
bolt snapshot create --with-gpu-state --name cp2077-session
```

### Counter-Strike 2

```bash
# Competitive FPS setup with minimum latency
bolt gaming realtime --enable

bolt run --gpus 1 \
  --name cs2 \
  --network host \
  -v ~/.steam:/steam \
  -e SDL_VIDEODRIVER=wayland \
  --device /dev/dri \
  --device /dev/input \
  steam-runtime cs2.sh +exec autoexec.cfg

# Apply low-latency optimizations
bolt gaming optimize --pid $(pgrep cs2)
```

---

## Benchmarking

### Built-in Benchmarks

```bash
# Run gaming benchmark suite
bolt gaming benchmark

# Tests:
# - GPU passthrough latency
# - Frame time consistency
# - Input lag
# - CPU overhead
# - Memory latency

# Compare with Docker
bolt gaming benchmark --compare-docker
```

### Custom Game Benchmarks

```bash
# Run game with performance logging
bolt run --gpus 1 \
  --name benchmark \
  -e ENABLE_PERF_LOG=1 \
  bolt/wine-proton:8.0 \
  wine game.exe +benchmark

# Extract results
bolt logs benchmark | grep "FPS:"
```

---

## Troubleshooting

### Game Won't Launch

```bash
# Check GPU access
bolt exec -it game-container nvidia-smi

# Verify Wine/Proton
bolt exec -it game-container wine --version

# Check logs
bolt logs game-container --follow
```

### Poor Performance

```bash
# Verify GPU is being used
bolt gpu metrics --container game-container

# Enable real-time optimizations
bolt gaming realtime --enable

# Check CPU affinity
bolt gaming optimize --pid <game_pid>

# Verify NVIDIA settings
nvidia-settings -q all | grep -i performance
```

### Audio Issues

```bash
# Check audio system
pactl info  # For PulseAudio/PipeWire

# Reconfigure audio
bolt gaming audio --system pipewire

# Test audio in container
bolt exec -it game-container speaker-test
```

### Input Lag

```bash
# Enable low-latency mode
bolt gaming realtime --enable

# Verify performance
bolt gaming performance

# Check for frame drops
bolt gpu metrics --container game
```

---

## Advanced Configurations

### Custom Boltfile for Gaming

```toml
# Boltfile.toml
[service.gaming]
image = "bolt/wine-proton:8.0"
gpus = 1
volumes = [
    "~/Games:/games",
    "~/.steam:/steam",
    "/run/user/1000/pipewire-0:/run/pipewire"
]
devices = ["/dev/dri", "/dev/input", "/dev/snd"]
network = "host"
env = {
    PROTON_VERSION = "8.0",
    DXVK_HUD = "fps",
    WINE_LARGE_ADDRESS_AWARE = "1"
}
realtime = true
cpu_affinity = "performance"

[service.gaming.nvidia]
dlss_mode = "Quality"
raytracing = true
shader_cache = true

[service.gaming.audio]
system = "pipewire"
low_latency = true
```

```bash
# Launch gaming environment
bolt surge up gaming
```

---

## Best Practices

### 1. Always Use Real-Time Mode for Gaming

```bash
bolt gaming realtime --enable
```

### 2. Pin Game to Performance Cores

```bash
bolt gaming optimize --pid <game_pid>
```

### 3. Use Snapshots for Save States

```bash
bolt snapshot create --with-gpu-state --name "game-session-$(date +%Y%m%d)"
```

### 4. Monitor GPU Temperature

```bash
bolt gpu metrics --interval 1
```

### 5. Use Dedicated GPU for Gaming

```bash
# Reserve GPU 0 for gaming only
bolt run --gpus exclusive --gpus device=0 game
```

---

## Integration with GhostForge

For the complete gaming experience, install [GhostForge](https://github.com/yourusername/ghostforge):

```bash
# Install GhostForge
cargo install ghostforge

# GhostForge provides:
# - Beautiful game library UI
# - Per-game optimization profiles
# - Performance overlay (FPS, temps, latency)
# - Auto-benchmarking
# - Game state management
# - Community sharing of optimal settings

# Launch game via GhostForge (uses Bolt backend)
ghostforge launch "Cyberpunk 2077"
```

GhostForge uses Bolt's runtime and GPU features while providing a user-friendly gaming interface.

---

## Next Steps

- **AI Workloads**: [AI_WORKLOADS.md](./AI_WORKLOADS.md)
- **Migration Guide**: [MIGRATION.md](./MIGRATION.md)
- **Advanced Features**: [ADVANCED.md](./ADVANCED.md)

---

*Game at peak performance with Bolt!* 🎮
