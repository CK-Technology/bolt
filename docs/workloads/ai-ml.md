# AI/ML Workloads

Bolt provides optimized GPU profiles for AI/ML inference and training.

## Quick Start

```bash
# Run Ollama with AI profile
bolt run --gpu all --gpu-profile ollama-medium ollama/ollama

# List AI profiles
bolt nv profile list --profile-type ai

# Check GPU compute capability
bolt nv info --detailed
```

## AI Profiles

Pre-configured profiles for common workloads:

| Profile | Use Case | Flash Attention | Tensor Parallel | Memory |
|---------|----------|-----------------|-----------------|--------|
| ollama-small | 7B models | Off | Off | Conservative |
| ollama-medium | 13B models | On | Off | Moderate |
| ollama-large | 70B+ models | On | On | Maximum |
| training-single | Single GPU training | On | Off | Maximum |
| training-multi | Multi-GPU training | On | On | Maximum |
| inference-batch | Batch inference | On | Off | Moderate |

### View Profile Details
```bash
bolt nv profile show ollama-large
```

### Apply Profile
```bash
bolt nv profile apply ollama-medium --output ai.json
```

## Ollama Setup

### Single Container
```bash
bolt run \
  --gpu all \
  --gpu-profile ollama-medium \
  -p 11434:11434 \
  -v ollama-models:/root/.ollama \
  ollama/ollama
```

### With Web UI (Boltfile.toml)
```toml
project = "ollama-stack"

[services.ollama]
image = "ollama/ollama"
ports = ["11434:11434"]
volumes = ["ollama-models:/root/.ollama"]

[services.ollama.gpu]
devices = "all"
profile = "ollama-medium"

[services.webui]
image = "ghcr.io/open-webui/open-webui:main"
ports = ["3000:8080"]
depends_on = ["ollama"]
environment = { OLLAMA_BASE_URL = "http://ollama:11434" }

[volumes.ollama-models]
driver = "local"
```

```bash
bolt surge up -d
```

## Training

### Single GPU
```bash
bolt run \
  --gpu 0 \
  --gpu-profile training-single \
  -v ./data:/data \
  -v ./output:/output \
  pytorch/pytorch:latest \
  python train.py
```

### Multi-GPU
```bash
bolt run \
  --gpu all \
  --gpu-profile training-multi \
  -v ./data:/data \
  pytorch/pytorch:latest \
  torchrun --nproc_per_node=2 train.py
```

## Inference

### Batch Processing
```bash
bolt run \
  --gpu all \
  --gpu-profile inference-batch \
  -v ./models:/models \
  -v ./input:/input \
  -v ./output:/output \
  my-inference-image
```

### API Server
```toml
[services.inference-api]
image = "my-model-server"
ports = ["8000:8000"]

[services.inference-api.gpu]
devices = "all"
profile = "inference-batch"
```

## Environment Variables

### CUDA
```bash
CUDA_VISIBLE_DEVICES=0,1      # GPU selection
CUDA_DEVICE_ORDER=PCI_BUS_ID  # Consistent ordering
```

### PyTorch
```bash
PYTORCH_CUDA_ALLOC_CONF=max_split_size_mb:512
TORCH_CUDA_ARCH_LIST="8.0;8.6;8.9;9.0"
```

### TensorFlow
```bash
TF_FORCE_GPU_ALLOW_GROWTH=true
TF_GPU_ALLOCATOR=cuda_malloc_async
```

## Memory Management

### Large Models
For models that exceed single GPU memory:

```toml
[services.llm.gpu]
devices = "all"
profile = "ollama-large"

[services.llm.environment]
OLLAMA_NUM_PARALLEL = "1"
OLLAMA_MAX_LOADED_MODELS = "1"
```

### Monitoring
```bash
# Watch GPU memory
watch -n 1 nvidia-smi

# Inside container
bolt exec my-container nvidia-smi
```

## Troubleshooting

### Out of Memory
- Use a smaller profile (ollama-small instead of medium)
- Reduce batch size
- Enable model quantization (Q4, Q8)

### Slow Inference
```bash
# Verify Flash Attention enabled
bolt nv profile show your-profile

# Check GPU utilization
nvidia-smi dmon
```

### CUDA Version Mismatch
```bash
# Check driver CUDA version
bolt nv info

# Match container CUDA version
bolt run --gpu all nvidia/cuda:12.0-base nvidia-smi
```
