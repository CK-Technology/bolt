# 🎮 Bolt GPU Support

Bolt includes comprehensive GPU support with the **Velocity runtime** - our native, high-performance container runtime with first-class support for NVIDIA's open-source kernel modules.

## 🚀 Features

### Dual Runtime Support
- **⚡ Velocity (Bolt Native)**: Our high-performance built-in GPU runtime
- **🐳 nvidia-container-runtime**: Full compatibility with existing NVIDIA toolkit

### GPU Discovery
- **🔬 NVML Integration**: Preferred method using NVIDIA Management Library
- **🖥️ nvidia-smi Fallback**: Command-line tool integration
- **📁 sysfs Detection**: Direct hardware discovery as last resort

### Driver Support (Priority Order)
- **⚡ NVIDIA Open GPU Kernel Modules**: Primary choice - supports Turing+ GPUs with full NVIDIA stack
  - Open-source kernel modules with NVIDIA userspace drivers
  - GSP firmware support for enhanced features
  - Full Vulkan, CUDA, and gaming support
  - Supports RTX 20/30/40 series, GTX 16 series, and professional GPUs
- **🔵 NVIDIA Proprietary**: Traditional nvidia.ko driver
  - Full feature support for all NVIDIA GPUs
  - Closed-source kernel modules
- **🟡 nouveau (Legacy)**: Community open-source driver
  - Limited feature set, basic OpenGL support
  - Mesa/Gallium3D implementation
- **🟠 NVK**: Community Vulkan driver for nouveau
  - Experimental Vulkan support on nouveau

### Container Types
- **👤 Rootless Containers**: Optimized for non-root execution
- **🔐 Privileged Containers**: Full GPU passthrough
- **🏗️ User Namespaces**: Secure GPU device mapping

## 📋 Configuration

### Basic NVIDIA Setup
```toml
[services.game.gaming.gpu.nvidia]
device = 0              # GPU device ID
dlss = true             # Enable DLSS
raytracing = true       # Enable RTX
cuda = false            # Enable CUDA compute
```

### AMD Setup
```toml
[services.compute.gaming.gpu.amd]
device = 0              # GPU device ID
rocm = true             # Enable ROCm
```

### GPU Passthrough
```toml
[services.highperf.gaming.gpu]
passthrough = true      # Maximum performance mode
```

## 🛠️ Usage

### Automatic Detection
```rust
use bolt::runtime::gpu::GPUManager;

let gpu_manager = GPUManager::new()?;
let gpus = gpu_manager.get_available_gpus().await?;

for gpu in &gpus {
    println!("Found: {} {} ({}MB)", gpu.vendor, gpu.name, gpu.memory_mb);
}
```

### Runtime Preference
```rust
// Prefer nvidia-container-runtime when available
gpu_manager.setup_gpu_with_runtime_preference(
    "my-container",
    &gpu_config,
    true  // prefer nvidia-container-runtime
).await?;

// Use bolt native Velocity runtime
gpu_manager.setup_gpu_with_runtime_preference(
    "my-container",
    &gpu_config,
    false  // prefer bolt native
).await?;
```

### Rootless Support Check
```rust
let support = gpu_manager.check_rootless_gpu_support().await?;

if support.is_rootless && !support.dri_access {
    support.print_suggestions(); // Shows setup improvements
}
```

## 🎯 Gaming Optimizations

### Wine/Proton Integration
- **🍷 NVAPI Support**: For Windows games via Wine/Proton
- **⚡ DXVK Integration**: DirectX to Vulkan translation
- **🎮 VKD3D Support**: DirectX 12 to Vulkan
- **🌟 Automatic Detection**: Chooses optimal driver path

### Performance Features
- **⚡ GameMode Integration**: CPU/GPU performance optimization
- **🥽 VR Support**: OpenVR and SteamVR compatibility
- **🖼️ Multi-GPU**: Support for multiple GPU setups
- **📊 Real-time Monitoring**: GPU utilization tracking

## 🔧 Device Access

### NVIDIA Open GPU Kernel Modules
```
/dev/nvidiactl        # Control device
/dev/nvidia-uvm       # Unified memory
/dev/nvidia0          # GPU 0
/dev/dri/card0        # Display (enhanced with open modules)
/dev/dri/renderD128   # Vulkan/compute (full feature set)
# GSP firmware loaded automatically
```

### NVIDIA Proprietary
```
/dev/nvidiactl        # Control device
/dev/nvidia-uvm       # Unified memory
/dev/nvidia0          # GPU 0
/dev/dri/renderD128   # Vulkan/compute
```

### nouveau/NVK (Legacy)
```
/dev/dri/card0        # Display
/dev/dri/renderD128   # Vulkan/compute (limited)
```

### AMD
```
/dev/dri/card0        # Display
/dev/dri/renderD128   # Vulkan/compute
```

## ⚡ NVIDIA Open GPU Kernel Modules Setup

### Requirements
- **GPU Generation**: Turing or later (RTX 20/30/40 series, GTX 16 series)
- **Kernel**: Linux 4.15 or newer
- **Architecture**: x86_64 or aarch64
- **GSP Firmware**: Automatically loaded with driver

### Installation
```bash
# Install NVIDIA driver with open modules
sudo apt install nvidia-driver-XXX-open  # Ubuntu/Debian
# or
sudo dnf install nvidia-driver-XXX-open  # Fedora

# Verify installation
modinfo nvidia_drm
lsmod | grep nvidia
```

### Verification
```bash
# Check if open modules are loaded
ls /sys/module/nvidia_*

# Verify GSP firmware
dmesg | grep -i gsp

# Check GPU compatibility
nvidia-smi --query-gpu=name --format=csv
```

## 🧑 Rootless Setup

For optimal rootless GPU access:

```bash
# Add user to required groups
sudo usermod -a -G render $USER
sudo usermod -a -G video $USER

# Create NVIDIA udev rule (if using proprietary driver)
echo 'KERNEL=="nvidia*", MODE="0666"' | sudo tee /etc/udev/rules.d/70-nvidia.rules

# Reload and restart
sudo udevadm control --reload
sudo systemctl restart udev
```

## 🐳 nvidia-container-runtime Integration

Bolt automatically detects and integrates with nvidia-container-runtime:

```bash
# Check if available
/usr/bin/nvidia-container-runtime --version
/usr/bin/nvidia-container-toolkit --version

# Bolt will use it automatically for privileged containers
# Falls back to Velocity runtime for rootless or when unavailable
```

## 📊 Environment Variables

### NVIDIA Open GPU Kernel Modules
```bash
NVIDIA_VISIBLE_DEVICES=0,1    # GPU devices
NVIDIA_DRIVER_CAPABILITIES=compute,utility,graphics,video,display
NVIDIA_GSP_OPTIMIZATIONS=1    # GSP firmware optimizations
NVIDIA_OPEN_MODULE_FEATURES=1 # Open module features
NVIDIA_TURING_OPTIMIZATIONS=1 # Turing+ optimizations
__GL_THREADED_OPTIMIZATIONS=1
CUDA_CACHE_MAXSIZE=2147483648 # 2GB CUDA cache
```

### NVIDIA Proprietary
```bash
NVIDIA_VISIBLE_DEVICES=0,1    # GPU devices
NVIDIA_DRIVER_CAPABILITIES=compute,utility,graphics
NVIDIA_ENABLE_DLSS=1          # Enable DLSS
NVIDIA_ENABLE_RTX=1           # Enable ray tracing
```

### Mesa/nouveau Configuration
```bash
MESA_LOADER_DRIVER_OVERRIDE=nouveau
GALLIUM_DRIVER=nouveau
VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/nouveau_icd.x86_64.json
```

## 🧪 Testing

Run the GPU test example:
```bash
cargo run --example gpu_test --features nvidia-support
```

## 🔍 Troubleshooting

### Common Issues

**No GPUs detected:**
- Check `nvidia-smi` or `lspci | grep VGA`
- Verify drivers are installed
- Check device permissions

**Rootless access denied:**
- Follow rootless setup steps above
- Check group memberships: `groups $USER`
- Verify udev rules are applied

**Performance issues:**
- Enable GPU passthrough for maximum performance
- Use privileged containers when security allows
- Check for resource limits

### Debug Information
```bash
# Enable debug logging
RUST_LOG=debug bolt run my-container

# Check GPU devices
ls -la /dev/nvidia* /dev/dri/*

# Verify permissions
id; groups
```

## 🎉 Features Implemented

✅ **NVML-based GPU discovery with sysfs fallback**
✅ **Device injection for NVIDIA proprietary drivers**
✅ **Support for open/NVK path via DRI devices**
✅ **Rootless container GPU access**
✅ **Dual nvidia-container-runtime and Velocity support**
✅ **Gaming optimizations and Wine integration**
✅ **Real-time GPU monitoring and management**