# 🚀 Bolt Container Examples

Welcome to Bolt's example configurations! These examples showcase Bolt's advanced features including GPU acceleration, AI workloads, gaming optimizations, and high-performance networking.

## 🤖 AI & Machine Learning Examples

### Ollama GPU-Accelerated AI (ollama-gpu.bolt.toml)

**Features:**
- 🎯 **NVIDIA Open GPU Kernel Modules** support (primary choice)
- ⚡ **Velocity Runtime** - Bolt's native container runtime
- 🧠 **AI Workload Optimization** with flash attention and KV caching
- 🌐 **QUIC Networking** for low-latency API calls
- 🔒 **Rootless Containers** for security
- 💾 **Bolt Storage** with optimized I/O

**Quick Start:**
```bash
# Pull models
mkdir -p models webui-data

# Run with GPU
bolt run ollama-gpu.bolt.toml

# Access Ollama API
curl http://localhost:11434/api/version

# Access Web UI
open http://localhost:3000
```

**GPU Requirements:**
- NVIDIA GPU with Compute Capability 7.0+ (Volta, Turing, Ampere, Ada, Hopper)
- NVIDIA Open GPU Kernel Modules (preferred) or proprietary driver
- 8GB+ GPU memory recommended

### Ollama CPU-Only (ollama-cpu.bolt.toml)

**Features:**
- 🔧 **CPU-Optimized** inference with aggressive quantization
- 📦 **Smaller Models** (8B parameters) for faster CPU inference
- ⚡ **Velocity Runtime** optimizations
- 🌐 **QUIC Networking**

**Quick Start:**
```bash
# Run CPU-only version
bolt run ollama-cpu.bolt.toml

# Pull a smaller model
curl -X POST http://localhost:11434/api/pull \
     -d '{"name": "llama3.1:8b"}'
```

## 🎮 Gaming Examples

### Gaming with Wayland + KDE (gaming-wayland-kde.bolt.toml)

**Features:**
- 🎮 **Gaming Workload** with GameMode integration
- 🌊 **Wayland Compositor** optimizations
- 🔷 **KDE/Plasma** specific gaming enhancements
- 📺 **VRR & HDR** support
- 🎯 **NVIDIA DLSS & Ray Tracing** enabled
- ⚡ **Low-Latency** optimizations

**Gaming Technologies Supported:**
- **Native Linux Games** (Steam, GOG, etc.)
- **Wine/Proton** with DXVK/VKD3D
- **VR Gaming** support
- **Multi-Monitor** setups

**KDE Gaming Features:**
- KWin gaming mode with VRR
- Plasma gaming optimizations
- Hardware acceleration
- Low-latency input handling

## 🚀 Performance Benefits

### Why Choose Bolt?

1. **🔥 Zero-Copy GPU Operations** (upcoming)
2. **⚡ QUIC-Based Networking** - 50% less latency
3. **🌊 Wayland Gaming Integration** - Native support
4. **🤖 AI-First Design** - Optimized for modern workloads
5. **🔒 Safe by Default** - Rootless containers, safe environment management
6. **📦 Velocity Runtime** - Bolt's native container runtime

### Performance Comparisons

| Feature | Docker/Podman | Bolt |
|---------|---------------|------|
| GPU Setup | Manual, complex | Automatic detection |
| AI Workloads | Basic support | AI-optimized |
| Gaming | Limited | Wayland + KDE optimized |
| Networking | TCP only | TCP + QUIC |
| Safety | Traditional | Memory-safe Rust |

## 🛠 Installation & Requirements

### System Requirements

**For AI Examples:**
- Linux with NVIDIA GPU (recommended) or powerful CPU
- 16GB+ RAM for GPU, 32GB+ for CPU inference
- NVIDIA driver 525+ or NVIDIA Open GPU Kernel Modules

**For Gaming Examples:**
- Linux with GPU (NVIDIA/AMD/Intel)
- Wayland compositor (KDE/GNOME/Sway/Hyprland)
- 16GB+ RAM
- Gaming-capable GPU

### Installation

```bash
# Install Bolt
curl -sSL https://install.bolt.rs | sh

# Verify installation
bolt --version

# Check GPU support
bolt gpu info

# Test examples
cd examples/
bolt run ollama-gpu.bolt.toml
```

## 🔧 Advanced Configuration

### GPU Configuration Options

```toml
[services.app.gpu]
nvidia = {
    device = 0,           # GPU index
    dlss = true,          # Enable DLSS
    raytracing = true,    # Enable RT cores
    cuda = true           # Enable CUDA
}
runtime_preference = "velocity"  # bolt | nvidia-runtime
passthrough = false              # Use integrated approach
```

### AI Workload Tuning

```toml
[services.app.ai_config]
backend = "ollama"              # ollama | localai | vllm
context_length = 8192           # Context window
batch_size = 4                  # Batch processing
quantization = "fp16"           # fp16 | int8 | int4
enable_flash_attention = true   # Memory-efficient attention
enable_kv_cache = true          # Key-value caching
```

### Gaming Optimizations

```toml
[services.app.wayland]
desktop_environment = "kde"     # kde | gnome | sway
enable_vrr = true              # Variable Refresh Rate
enable_hdr = true              # High Dynamic Range
enable_low_latency = true      # Gaming-focused latency
```

### Network Optimization

```toml
[services.app.network]
enable_quic = true             # QUIC protocol
low_latency = true             # Latency optimizations
bandwidth_optimization = true   # Throughput optimizations
```

## 🐛 Troubleshooting

### Common Issues

**GPU Not Detected:**
```bash
# Check GPU detection
bolt gpu info

# Verify drivers
nvidia-smi  # or amd-smi

# Check permissions
bolt gpu test-access
```

**AI Models Won't Load:**
```bash
# Check GPU memory
bolt gpu memory

# Verify model path
ls -la models/

# Check container logs
bolt logs ollama-gpu-ai
```

**Gaming Performance Issues:**
```bash
# Verify Wayland
echo $WAYLAND_DISPLAY

# Check desktop environment
echo $XDG_CURRENT_DESKTOP

# Gaming mode status
bolt gaming status
```

### Performance Tuning

**For AI Workloads:**
- Increase GPU memory allocation
- Enable tensor cores for compatible models
- Use fp16 quantization for speed
- Batch multiple requests

**For Gaming:**
- Enable VRR on compatible displays
- Use performance CPU governor
- Ensure GameMode is running
- Check for compositor optimizations

## 🤝 Contributing

Found an issue or want to contribute an example?

1. Fork the repository
2. Create your example
3. Test thoroughly
4. Submit a pull request

## 📚 Additional Resources

- [Bolt Documentation](https://docs.bolt.rs)
- [NVIDIA Open GPU Kernel Modules](https://github.com/NVIDIA/open-gpu-kernel-modules)
- [Wayland Gaming Guide](https://wiki.archlinux.org/title/Wayland#Gaming)
- [Ollama Documentation](https://ollama.ai/docs)

---

**Happy containerizing with Bolt! 🚀⚡**