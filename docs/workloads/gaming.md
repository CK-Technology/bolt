# Gaming

Bolt provides first-class gaming support with GPU profiles, Wine/Proton optimization, and low-latency configuration.

## Quick Start

```bash
# Run with gaming profile
bolt run --gpu all --gpu-profile "cyberpunk 2077" steam-image

# List gaming profiles
bolt nv profile list --profile-type gaming

# Check GPU readiness
bolt nv doctor
```

## Gaming Profiles

Pre-configured profiles for popular games:

| Profile | DLSS | Ray Tracing | Reflex | Notes |
|---------|------|-------------|--------|-------|
| cyberpunk 2077 | Quality | On | On | Path tracing optimized |
| doom eternal | Off | Off | Off | High FPS priority |
| hogwarts legacy | Quality | On | On | Balanced |
| fortnite | Performance | Off | On | Competitive |
| minecraft rtx | Performance | On | Off | RT shaders |
| elden ring | Balanced | Off | Off | Frame pacing |

### View Profile Details
```bash
bolt nv profile show "cyberpunk 2077"
```

### Apply Profile
```bash
# Apply and generate CDI spec
bolt nv profile apply "cyberpunk 2077" --output gaming.json
```

## GPU Configuration

### NVIDIA Features
```bash
# Check DLSS/RT support
bolt nv info --detailed

# Verify architecture
bolt nv arch
```

### Performance Modes
- **Ultra** - Maximum performance, highest power
- **Balanced** - Good performance, moderate power
- **Quiet** - Reduced noise, lower performance

## Container Setup

### Boltfile.toml
```toml
project = "gaming"

[services.steam]
image = "ghcr.io/games-on-whales/steam:latest"
ports = ["8080:8080"]

[services.steam.gpu]
devices = "all"
profile = "cyberpunk 2077"

[services.steam.gaming]
wine_optimizations = true
audio_system = "pipewire"
audio_latency = "low"
```

### Direct Run
```bash
bolt run \
  --gpu all \
  --gpu-profile "doom eternal" \
  -p 8080:8080 \
  ghcr.io/games-on-whales/steam:latest
```

## Wine/Proton

Bolt optimizes Wine/Proton containers:

- DXVK/VKD3D auto-configuration
- Shader cache management
- FSR/DLSS frame generation
- Gamemode integration

### Environment Variables
```bash
PROTON_VERSION=8.0
WINE_PREFIX=/home/user/.wine
DXVK_ASYNC=1
PROTON_ENABLE_NVAPI=1
```

## Audio

### PipeWire (Recommended)
```toml
[services.game.gaming]
audio_system = "pipewire"
audio_latency = "low"
```

### PulseAudio
```toml
[services.game.gaming]
audio_system = "pulseaudio"
```

## Display

### Wayland
```bash
bolt gaming wayland
```

### X11
```bash
bolt run -e DISPLAY=$DISPLAY -v /tmp/.X11-unix:/tmp/.X11-unix ...
```

## Troubleshooting

### Poor Performance
```bash
# Check GPU is detected
bolt nv info

# Verify profile applied
bolt nv profile show "your-profile"

# Check driver
bolt nv driver
```

### No Audio
```bash
# Verify PipeWire/PulseAudio socket
ls -la /run/user/$(id -u)/pipewire-0
ls -la /run/user/$(id -u)/pulse/native
```

### Input Lag
- Use Reflex-enabled profiles
- Enable `--gpu-profile` with low-latency settings
- Check compositor bypass settings
