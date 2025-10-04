# 🤖 AI/ML Workloads with Bolt

Complete guide to running AI and machine learning workloads on Bolt.

---

## Why Bolt for AI/ML?

- **130x Faster GPU Passthrough**: Sub-microsecond GPU access vs Docker's 104μs overhead
- **Intelligent Multi-GPU Scheduling**: Automatic GPU allocation based on workload requirements
- **Model Caching**: HuggingFace Hub integration with content-addressable deduplication
- **vLLM Integration**: Optimized LLM serving with tensor parallelism
- **MIG Support**: A100/H100 GPU partitioning for efficient resource utilization

---

## LLM Inference

### Serve an LLM with vLLM

```bash
# Serve Llama 3 8B (single GPU)
bolt serve \
  --model meta-llama/Llama-3-8B \
  --gpus 1 \
  --port 8000

# Serve Llama 3 70B (multi-GPU with tensor parallelism)
bolt serve \
  --model meta-llama/Llama-3-70B \
  --gpus 4 \
  --backend vllm \
  --tensor-parallel 4 \
  --port 8000

# Serve Mixtral 8x7B with optimizations
bolt serve \
  --model mistralai/Mixtral-8x7B-Instruct-v0.1 \
  --gpus 2 \
  --max-batch-size 128 \
  --max-num-seqs 256 \
  --port 8000
```

### OpenAI-Compatible API

Once serving, use the OpenAI-compatible API:

```bash
# Chat completion
curl http://localhost:8000/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "meta-llama/Llama-3-70B",
    "messages": [{"role": "user", "content": "Explain quantum computing"}],
    "temperature": 0.7,
    "max_tokens": 512
  }'

# Streaming response
curl http://localhost:8000/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "meta-llama/Llama-3-70B",
    "messages": [{"role": "user", "content": "Write a story"}],
    "stream": true
  }'
```

### Python Client Example

```python
from openai import OpenAI

client = OpenAI(
    base_url="http://localhost:8000/v1",
    api_key="not-needed"  # vLLM doesn't require API key
)

response = client.chat.completions.create(
    model="meta-llama/Llama-3-70B",
    messages=[
        {"role": "system", "content": "You are a helpful assistant."},
        {"role": "user", "content": "Explain transformers in ML"}
    ],
    temperature=0.7,
    max_tokens=1024
)

print(response.choices[0].message.content)
```

---

## Model Training

### PyTorch Training

```bash
# Single GPU training
bolt run --gpus 1 \
  -v $(pwd):/workspace \
  pytorch/pytorch:latest \
  python train.py

# Multi-GPU distributed training (4 GPUs)
bolt run --gpus 4 \
  -v $(pwd):/workspace \
  -e WORLD_SIZE=4 \
  pytorch/pytorch:latest \
  torchrun --nproc_per_node=4 train.py

# With specific GPU selection
bolt run --gpus device=0,2 \
  -v $(pwd):/workspace \
  pytorch/pytorch:latest \
  python train.py
```

### TensorFlow Training

```bash
# Single GPU
bolt run --gpus 1 \
  -v $(pwd):/workspace \
  tensorflow/tensorflow:latest-gpu \
  python train_tf.py

# Multi-GPU MirroredStrategy
bolt run --gpus 4 \
  -v $(pwd):/workspace \
  -e TF_FORCE_GPU_ALLOW_GROWTH=true \
  tensorflow/tensorflow:latest-gpu \
  python train_distributed.py
```

### JAX/Flax Training

```bash
bolt run --gpus 2 \
  -v $(pwd):/workspace \
  ghcr.io/google/jax:latest \
  python train_jax.py
```

---

## Image Generation

### Stable Diffusion

```bash
# Pull model from HuggingFace
bolt model pull huggingface:stabilityai/stable-diffusion-xl-base-1.0

# Serve with automatic1111 WebUI
bolt run --gpus 1 \
  -p 7860:7860 \
  -v ~/.cache/bolt/models:/models \
  --name sd-webui \
  ghcr.io/automatic1111/stable-diffusion-webui:latest

# Access at http://localhost:7860
```

### ComfyUI

```bash
bolt run --gpus 1 \
  -p 8188:8188 \
  -v $(pwd)/models:/models \
  -v $(pwd)/output:/output \
  --name comfyui \
  ghcr.io/comfyanonymous/comfyui:latest
```

---

## Multi-GPU Scheduling

### GPU Allocation Strategies

Bolt intelligently schedules workloads across multiple GPUs:

```bash
# Configure scheduling strategy
bolt gpu config --strategy least-utilized

# Available strategies:
# - round-robin: Rotate through GPUs
# - least-utilized: Choose GPU with lowest utilization
# - most-memory: Choose GPU with most available VRAM
# - exclusive: Each container gets dedicated GPUs
```

### Running Multiple Workloads

```bash
# Start multiple AI workloads efficiently
bolt run --gpus 2 --name training-job pytorch train.py
bolt run --gpus 1 --name inference-1 vllm serve model1
bolt run --gpus 1 --name inference-2 vllm serve model2

# Check GPU allocation
bolt gpu status

# Output:
# ╔═══════╦════════════╦══════════════════════╦═════════╗
# ║  GPU  ║ Container  ║     Utilization      ║  Memory ║
# ╠═══════╬════════════╬══════════════════════╬═════════╣
# ║ gpu:0 ║ training   ║ ████████████░░ 85%   ║ 20/24GB ║
# ║ gpu:1 ║ training   ║ ████████████░░ 82%   ║ 19/24GB ║
# ║ gpu:2 ║ inference1 ║ ████░░░░░░░░░░ 35%   ║  8/24GB ║
# ║ gpu:3 ║ inference2 ║ ███░░░░░░░░░░░ 28%   ║  6/24GB ║
# ╚═══════╩════════════╩══════════════════════╩═════════╝
```

---

## MIG (Multi-Instance GPU)

For A100/H100 GPUs, use MIG to partition GPUs:

```bash
# Create MIG instance (1g.5gb profile)
bolt gpu mig --gpu 0 --profile 1g.5gb

# Run container with MIG instance
bolt run --gpus mig:1g.5gb pytorch train.py

# Available MIG profiles:
# A100-80GB:
#   - 1g.10gb  (1 slice, 10GB)
#   - 2g.20gb  (2 slices, 20GB)
#   - 3g.40gb  (3 slices, 40GB)
#   - 7g.80gb  (7 slices, 80GB)
#
# A100-40GB:
#   - 1g.5gb   (1 slice, 5GB)
#   - 2g.10gb  (2 slices, 10GB)
#   - 3g.20gb  (3 slices, 20GB)
#   - 7g.40gb  (7 slices, 40GB)
```

---

## Model Management

### Download Models

```bash
# Pull from HuggingFace
bolt model pull huggingface:meta-llama/Llama-3-70B
bolt model pull huggingface:mistralai/Mixtral-8x7B
bolt model pull huggingface:stabilityai/stable-diffusion-xl

# List cached models
bolt model list

# Output:
# ╔════════════════════════════════════════╦══════════╦═══════════════════╗
# ║ Model ID                               ║   Size   ║  Last Accessed    ║
# ╠════════════════════════════════════════╬══════════╬═══════════════════╣
# ║ meta-llama/Llama-3-70B                 ║  140 GB  ║  2 hours ago      ║
# ║ mistralai/Mixtral-8x7B                 ║   87 GB  ║  1 day ago        ║
# ║ stabilityai/stable-diffusion-xl        ║    7 GB  ║  3 days ago       ║
# ╚════════════════════════════════════════╩══════════╩═══════════════════╝
```

### Model Deduplication

Bolt automatically deduplicates common model files:

```bash
# Models share common files (tokenizers, configs)
# Bolt uses content-addressable storage with hard links
# Save 50%+ storage for models from same family

bolt model stats
# Output:
# Total models: 12
# Total size: 850 GB
# After deduplication: 520 GB (39% savings)
```

### Prune Unused Models

```bash
# Remove models not accessed in 30 days
bolt model prune --older-than 30d

# Remove specific model
bolt model rm huggingface:old-model-v1
```

---

## Advanced Configurations

### VRAM Quotas

```bash
# Limit container VRAM usage
bolt run --gpus 1 --gpu-memory 16GB pytorch train.py

# Split GPU between containers
bolt run --gpus 1 --gpu-memory 12GB --name job1 pytorch train1.py
bolt run --gpus 1 --gpu-memory 12GB --name job2 pytorch train2.py
```

### Custom Model Serving Backends

```bash
# TensorRT for optimized inference
bolt serve \
  --model /path/to/model.onnx \
  --backend tensorrt \
  --precision fp16 \
  --gpus 1 \
  --port 8001

# ONNX Runtime with CUDA
bolt serve \
  --model /path/to/model.onnx \
  --backend onnx \
  --execution-provider CUDA \
  --gpus 1 \
  --port 8002
```

### Health Checks and Auto-Restart

```bash
# Enable health checks for model servers
bolt serve \
  --model meta-llama/Llama-3-8B \
  --gpus 1 \
  --enable-healthcheck \
  --auto-restart \
  --port 8000

# Server automatically restarts on failure
# Health checks every 30s at /health endpoint
```

---

## Boltfile for AI Projects

```toml
# Boltfile.toml
[service.training]
image = "pytorch/pytorch:latest"
gpus = 2
volumes = ["./data:/data", "./checkpoints:/checkpoints"]
env = { CUDA_VISIBLE_DEVICES = "0,1" }
command = ["python", "train.py"]

[service.inference]
image = "vllm/vllm-openai:latest"
gpus = 1
ports = ["8000:8000"]
env = {
    MODEL = "meta-llama/Llama-3-8B",
    TENSOR_PARALLEL_SIZE = "1"
}
healthcheck = { enabled = true, interval = "30s" }
auto_restart = true

[service.notebook]
image = "jupyter/tensorflow-notebook:latest"
gpus = 1
ports = ["8888:8888"]
volumes = ["./notebooks:/notebooks"]
```

```bash
# Start all services
bolt surge up

# Start specific service
bolt surge up inference

# View logs
bolt surge logs inference --follow

# Scale inference servers
bolt surge scale inference=3

# Stop all
bolt surge down
```

---

## Performance Monitoring

### Real-Time GPU Metrics

```bash
# Monitor all GPUs
bolt gpu metrics

# Monitor specific container
bolt gpu metrics --container training-job

# Set update interval
bolt gpu metrics --interval 1  # Update every 1 second
```

### Integration with Monitoring Tools

```bash
# Prometheus metrics endpoint
bolt serve \
  --model meta-llama/Llama-3-70B \
  --gpus 4 \
  --enable-metrics \
  --metrics-port 9090

# Metrics available at http://localhost:9090/metrics
# - vllm_request_latency
# - vllm_tokens_per_second
# - gpu_utilization
# - gpu_memory_used
```

---

## Best Practices

### 1. GPU Selection

```bash
# Let Bolt choose optimal GPUs
bolt run --gpus 2 pytorch train.py

# Specify exact GPUs only when needed
bolt run --gpus device=0,1 pytorch train.py
```

### 2. Memory Management

```bash
# Enable gradient checkpointing for large models
bolt run --gpus 2 \
  -e PYTORCH_CUDA_ALLOC_CONF=max_split_size_mb:512 \
  pytorch train_large_model.py
```

### 3. Model Caching

```bash
# Pre-download models before training
bolt model pull huggingface:meta-llama/Llama-3-70B

# Mount model cache as read-only
bolt run --gpus 4 \
  -v ~/.cache/bolt/models:/models:ro \
  pytorch train.py
```

### 4. Efficient Multi-Job Scheduling

```bash
# Use least-utilized strategy for dynamic workloads
bolt gpu config --strategy least-utilized

# Reserve GPUs for critical jobs
bolt run --gpus exclusive --name critical-training pytorch train.py
```

---

## Troubleshooting

### GPU Not Detected

```bash
# Check GPU availability
bolt gpu list

# If empty, verify drivers
nvidia-smi

# Check nvbind installation
bolt gpu check
```

### Out of Memory Errors

```bash
# Reduce batch size or enable gradient checkpointing
# Use MIG for smaller workloads
bolt run --gpus mig:1g.5gb pytorch train.py

# Monitor memory usage
bolt gpu metrics --container training-job
```

### Model Download Issues

```bash
# Set HuggingFace token for gated models
export HF_TOKEN=your_token_here

bolt model pull huggingface:meta-llama/Llama-3-70B

# Use mirror for faster downloads
export HF_ENDPOINT=https://hf-mirror.com
```

---

## Example Workflows

### Fine-Tuning LLM

```bash
# 1. Download base model
bolt model pull huggingface:meta-llama/Llama-3-8B

# 2. Start training with 4 GPUs
bolt run --gpus 4 \
  -v $(pwd)/data:/data \
  -v $(pwd)/output:/output \
  -v ~/.cache/bolt/models:/models:ro \
  --name finetune \
  pytorch/pytorch:latest \
  accelerate launch --multi_gpu --num_processes=4 finetune.py

# 3. Monitor training
bolt logs finetune --follow
bolt gpu metrics --container finetune

# 4. Serve fine-tuned model
bolt serve \
  --model /output/checkpoint-1000 \
  --gpus 2 \
  --port 8000
```

### Batch Inference

```bash
# Run inference on large dataset
bolt run --gpus 2 \
  -v $(pwd)/input:/input:ro \
  -v $(pwd)/output:/output \
  --name batch-inference \
  pytorch/pytorch:latest \
  python batch_infer.py --input /input --output /output
```

---

## Next Steps

- **Migration Guide**: [MIGRATION.md](./MIGRATION.md)
- **Advanced Features**: [ADVANCED.md](./ADVANCED.md)
- **Gaming Setup**: [GAMING.md](./GAMING.md)

---

*Maximize your AI/ML performance with Bolt!* 🚀
