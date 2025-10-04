# 🧠 Bolt AI Architecture
**High-Performance AI/ML Container Runtime**

---

## 🎯 Vision

**Make Bolt the best container runtime for AI/ML workloads by solving the hardest GPU scheduling, model serving, and distributed training problems.**

### What Makes Bolt Different for AI?

| Feature | Docker | Podman | Kubernetes | **Bolt** |
|---------|--------|--------|------------|----------|
| **GPU Scheduling** | Manual | Basic CDI | Device plugin | ✅ Intelligent scheduler |
| **Multi-GPU** | One per container | Limited | Complex setup | ✅ Automatic allocation |
| **Model Serving** | DIY | DIY | KServe (complex) | ✅ Built-in vLLM/TRT |
| **Distributed Training** | Manual | Manual | Kubeflow | ✅ One-command DDP |
| **GPU Memory** | No visibility | No visibility | Limited | ✅ VRAM tracking |
| **MIG Support** | No | No | Limited | ✅ Full A100/H100 |
| **Startup Time** | ~500ms | ~400ms | ~2s | ✅ <100ms |
| **GPU Passthrough** | ~100μs | ~50μs | ~200μs | ✅ <1μs |

---

## 🏗️ System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Bolt AI Runtime                         │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌─────────────────┐  │
│  │ GPU Scheduler│  │ Model Server │  │ Training Manager│  │
│  │              │  │              │  │                 │  │
│  │ • Round-robin│  │ • vLLM       │  │ • PyTorch DDP   │  │
│  │ • Load-aware │  │ • TensorRT   │  │ • DeepSpeed     │  │
│  │ • MIG support│  │ • ONNX       │  │ • Horovod       │  │
│  │ • Memory mgmt│  │ • Batch opt  │  │ • NCCL tuning   │  │
│  └──────────────┘  └──────────────┘  └─────────────────┘  │
│                                                              │
│  ┌─────────────────────────────────────────────────────┐   │
│  │           Model Registry & Cache                    │   │
│  │                                                      │   │
│  │  • HuggingFace Hub    • Local cache                │   │
│  │  • Memory mapping     • Deduplication              │   │
│  │  • Fast loading       • Version control            │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                              │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              Resource Manager                        │   │
│  │                                                      │   │
│  │  • VRAM quotas        • CPU/memory limits           │   │
│  │  • Network QoS        • Disk I/O limits             │   │
│  │  • Priority queuing   • Preemption support          │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                              │
├─────────────────────────────────────────────────────────────┤
│                   nvbind GPU Integration                     │
│  • <1μs GPU passthrough    • CDI device injection           │
│  • CUDA/ROCm env setup     • Driver compatibility           │
└─────────────────────────────────────────────────────────────┘
```

---

## 🔧 Component Deep Dive

### 1. GPU Scheduler

**Purpose:** Intelligently allocate GPUs across containers

#### Architecture
```rust
pub struct GpuScheduler {
    // GPU inventory
    gpus: Arc<RwLock<HashMap<String, GpuState>>>,

    // Allocation tracking
    allocations: Arc<RwLock<HashMap<String, Vec<String>>>>,

    // Scheduling policy
    strategy: SchedulingStrategy,

    // MIG manager
    mig_manager: Option<MigManager>,

    // Memory tracker
    memory_tracker: VramTracker,
}
```

#### Features

**1.1 Smart Allocation Strategies**
```rust
pub enum SchedulingStrategy {
    RoundRobin,          // Simple rotation
    LeastUtilized,       // Min GPU usage
    MostMemory,          // Max available VRAM
    Exclusive,           // No sharing
    TimeSlicing,         // Share GPUs with time-slicing
    BestFit,             // Pack containers efficiently
}
```

**1.2 MIG (Multi-Instance GPU) Support**
```rust
// A100/H100 GPU partitioning
pub struct MigManager {
    instances: HashMap<String, MigInstance>,
}

pub struct MigInstance {
    gpu_id: String,
    instance_id: u32,
    gpu_slices: u32,        // 1-7 slices
    memory_mb: u64,         // 5GB, 10GB, 20GB, 40GB, 80GB
    compute_slices: u32,    // 1-7 compute slices
    allocated_to: Option<String>,
}

// Usage:
bolt run --gpus mig:1g.5gb myimage  // Use 1 GPU slice, 5GB
bolt run --gpus mig:3g.20gb myimage // Use 3 GPU slices, 20GB
```

**1.3 Time-Slicing (GPU Sharing)**
```rust
pub struct TimeSlicingScheduler {
    slice_duration_ms: u64,     // 100ms per slice
    containers: Vec<String>,     // Containers sharing GPU
    current_slice: usize,
}

// Allows 10 containers to share 1 GPU
// Each gets 100ms every 1 second
```

**1.4 VRAM Tracking**
```rust
pub struct VramTracker {
    total_vram_mb: HashMap<String, u64>,
    used_vram_mb: HashMap<String, u64>,
    reserved_vram_mb: HashMap<String, u64>,
}

// Real-time VRAM monitoring
impl VramTracker {
    async fn get_free_vram(&self, gpu_id: &str) -> u64 {
        // Query nvidia-smi or nvml
        let total = self.total_vram_mb.get(gpu_id).unwrap();
        let used = self.get_current_usage(gpu_id).await;
        total - used
    }
}
```

---

### 2. Model Serving

**Purpose:** Production-grade LLM/diffusion model serving

#### Architecture
```rust
pub struct ModelServer {
    backend: ServingBackend,
    model_cache: ModelCache,
    batch_scheduler: BatchScheduler,
    kv_cache_manager: KVCacheManager,
}

pub enum ServingBackend {
    VLLM {
        tensor_parallel: u32,
        pipeline_parallel: u32,
        max_batch_size: u32,
    },
    TensorRT {
        engine_path: PathBuf,
        precision: Precision,  // FP16, INT8, INT4
    },
    ONNX {
        model_path: PathBuf,
        execution_provider: String,  // CUDA, TensorRT, ROCm
    },
}
```

#### Features

**2.1 vLLM Integration (LLM Inference)**
```bash
# Deploy Llama 3 70B with 4 GPUs
bolt serve \
  --model meta-llama/Llama-3-70B \
  --backend vllm \
  --tensor-parallel 4 \
  --max-batch-size 64 \
  --port 8000
```

**Backend implementation:**
```rust
impl ModelServer {
    async fn start_vllm(&self, config: VLLMConfig) -> Result<()> {
        let container_config = ContainerConfig {
            image: "vllm/vllm-openai:latest".to_string(),
            gpus: GpuRequest::Count(config.tensor_parallel as usize),
            env: vec![
                format!("MODEL={}", config.model_name),
                format!("TENSOR_PARALLEL_SIZE={}", config.tensor_parallel),
                format!("MAX_NUM_SEQS={}", config.max_batch_size),
                "TRUST_REMOTE_CODE=true".to_string(),
            ],
            ports: vec![format!("{}:8000", config.port)],
            shared_memory_size: Some("8G".to_string()),  // For tensor storage
            ..Default::default()
        };

        self.runtime.create_and_start_container("vllm-server", container_config).await?;

        // Wait for readiness
        self.wait_for_model_ready(config.port).await?;

        Ok(())
    }

    async fn wait_for_model_ready(&self, port: u16) -> Result<()> {
        for _ in 0..60 {  // 60 second timeout
            if self.check_health(port).await.is_ok() {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        Err(anyhow!("Model server failed to start"))
    }
}
```

**2.2 TensorRT Optimization**
```rust
// Automatic TensorRT conversion for 3-5x speedup
impl ModelServer {
    async fn optimize_with_tensorrt(&self, model: &str) -> Result<PathBuf> {
        // Convert ONNX/PyTorch → TensorRT engine
        let engine_path = self.model_cache.get_tensorrt_engine(model).await?;

        if !engine_path.exists() {
            info!("Building TensorRT engine for {}", model);
            self.build_tensorrt_engine(model, &engine_path).await?;
        }

        Ok(engine_path)
    }

    async fn build_tensorrt_engine(&self, model: &str, output: &Path) -> Result<()> {
        // Run trtexec in container
        let cmd = format!(
            "trtexec --onnx={} --saveEngine={} --fp16 --workspace=8000",
            model, output.display()
        );

        self.runtime.exec("tensorrt-builder", &cmd).await?;
        Ok(())
    }
}
```

**2.3 Dynamic Batching**
```rust
pub struct BatchScheduler {
    max_batch_size: usize,
    batch_timeout_ms: u64,
    pending_requests: Vec<InferenceRequest>,
}

impl BatchScheduler {
    async fn schedule(&mut self) -> Vec<InferenceRequest> {
        // Wait for batch to fill or timeout
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(self.batch_timeout_ms)) => {
                // Timeout: process current batch
                self.drain_batch()
            }
            _ = async {
                while self.pending_requests.len() < self.max_batch_size {
                    self.pending_requests.push(self.recv_request().await);
                }
            } => {
                // Batch full: process immediately
                self.drain_batch()
            }
        }
    }
}
```

**2.4 KV Cache Management (Transformer Optimization)**
```rust
pub struct KVCacheManager {
    cache_size_mb: u64,
    cache_blocks: Vec<CacheBlock>,
    lru_policy: LRUCache,
}

// Reuse KV cache across requests for same prompt prefix
impl KVCacheManager {
    async fn get_or_compute(&mut self, prompt: &str, model: &str) -> CacheBlock {
        let cache_key = self.compute_key(prompt, model);

        if let Some(block) = self.lru_policy.get(&cache_key) {
            // Cache hit: reuse KV cache
            return block.clone();
        }

        // Cache miss: compute and store
        let block = self.compute_kv_cache(prompt, model).await;
        self.lru_policy.insert(cache_key, block.clone());
        block
    }
}
```

---

### 3. Distributed Training

**Purpose:** Multi-node GPU training orchestration

#### Architecture
```rust
pub struct TrainingManager {
    cluster: ClusterManager,
    strategy: DistributionStrategy,
    communication: CommunicationBackend,
}

pub enum DistributionStrategy {
    DataParallel,           // PyTorch DDP
    TensorParallel,         // Megatron-LM
    PipelineParallel,       // GPipe
    ZeRO,                   // DeepSpeed ZeRO
    FSDP,                   // Fully Sharded Data Parallel
}

pub enum CommunicationBackend {
    NCCL,                   // NVIDIA NCCL
    Gloo,                   // CPU fallback
    MPI,                    // Message Passing Interface
}
```

#### Features

**3.1 PyTorch Distributed Data Parallel**
```bash
# Multi-node training with 32 GPUs
bolt train \
  --nodes 4 \
  --gpus-per-node 8 \
  --strategy ddp \
  --master-addr node-0.bolt.local \
  pytorch/pytorch:latest \
  python train.py
```

**Implementation:**
```rust
impl TrainingManager {
    async fn setup_ddp(&self, config: TrainingConfig) -> Result<()> {
        let world_size = config.nodes * config.gpus_per_node;

        for node_rank in 0..config.nodes {
            for local_rank in 0..config.gpus_per_node {
                let global_rank = node_rank * config.gpus_per_node + local_rank;

                let container_config = ContainerConfig {
                    image: config.image.clone(),
                    gpus: GpuRequest::Specific(vec![format!("gpu:{}", local_rank)]),
                    env: vec![
                        format!("WORLD_SIZE={}", world_size),
                        format!("RANK={}", global_rank),
                        format!("LOCAL_RANK={}", local_rank),
                        format!("MASTER_ADDR={}", config.master_addr),
                        format!("MASTER_PORT={}", config.master_port),
                        "NCCL_SOCKET_IFNAME=eth0".to_string(),
                        "NCCL_DEBUG=INFO".to_string(),
                    ],
                    command: config.command.clone(),
                    network: Some("bolt-ddp-network".to_string()),
                    ..Default::default()
                };

                let container_id = format!("trainer-{}-{}", node_rank, local_rank);
                self.runtime.create_and_start_container(&container_id, container_config).await?;
            }
        }

        Ok(())
    }
}
```

**3.2 DeepSpeed ZeRO Integration**
```rust
// Automatic ZeRO-3 configuration for large models
impl TrainingManager {
    async fn setup_deepspeed_zero3(&self, config: TrainingConfig) -> Result<()> {
        // Generate DeepSpeed config
        let ds_config = json!({
            "train_batch_size": config.batch_size * config.world_size,
            "gradient_accumulation_steps": config.gradient_accumulation,
            "zero_optimization": {
                "stage": 3,
                "offload_param": {"device": "cpu"},
                "offload_optimizer": {"device": "cpu"},
                "overlap_comm": true,
                "contiguous_gradients": true,
                "reduce_bucket_size": 5e8,
            },
            "fp16": {"enabled": true},
            "zero_allow_untested_optimizer": true,
        });

        // Inject DeepSpeed config into containers
        for container in &config.containers {
            self.runtime.exec(
                container,
                &format!("echo '{}' > /tmp/ds_config.json", ds_config)
            ).await?;
        }

        Ok(())
    }
}
```

**3.3 NCCL Auto-Tuning**
```rust
// Optimize NCCL for network topology
pub struct NCCLOptimizer {
    network_topology: NetworkTopology,
}

impl NCCLOptimizer {
    async fn tune(&self) -> HashMap<String, String> {
        let mut env = HashMap::new();

        // Auto-detect optimal settings
        if self.network_topology.has_infiniband() {
            env.insert("NCCL_IB_DISABLE".to_string(), "0".to_string());
            env.insert("NCCL_IB_HCA".to_string(), "mlx5_0,mlx5_1".to_string());
        }

        if self.network_topology.has_nvlink() {
            env.insert("NCCL_P2P_LEVEL".to_string(), "NVL".to_string());
        }

        // Tree/ring algorithm selection
        let num_gpus = self.network_topology.total_gpus();
        if num_gpus > 16 {
            env.insert("NCCL_ALGO".to_string(), "Tree".to_string());
        } else {
            env.insert("NCCL_ALGO".to_string(), "Ring".to_string());
        }

        env
    }
}
```

---

### 4. Model Registry & Cache

**Purpose:** Fast model loading and storage

#### Architecture
```rust
pub struct ModelCache {
    cache_dir: PathBuf,
    models: HashMap<String, CachedModel>,
    dedup_store: ContentAddressableStore,
}

pub struct CachedModel {
    model_id: String,
    files: Vec<CachedFile>,
    total_size_bytes: u64,
    last_accessed: SystemTime,
}

pub struct CachedFile {
    path: PathBuf,
    content_hash: String,  // SHA256
    is_deduplicated: bool,
    memory_mapped: bool,
}
```

#### Features

**4.1 HuggingFace Hub Integration**
```bash
# Download and cache models
bolt model pull huggingface:meta-llama/Llama-3-70B
bolt model pull huggingface:stabilityai/stable-diffusion-xl-base-1.0
```

```rust
impl ModelCache {
    async fn pull_from_huggingface(&mut self, model_id: &str) -> Result<()> {
        // Download using huggingface_hub API
        let files = self.download_model_files(model_id).await?;

        // Deduplicate common files (tokenizer, config)
        for file in &files {
            if let Some(dedup_path) = self.dedup_store.get(&file.content_hash) {
                // Hardlink instead of duplicate
                std::fs::hard_link(dedup_path, &file.path)?;
            } else {
                self.dedup_store.insert(&file.content_hash, &file.path);
            }
        }

        Ok(())
    }
}
```

**4.2 Memory-Mapped Loading (10x faster)**
```rust
impl ModelCache {
    async fn load_model_mmap(&self, model_id: &str) -> Result<MemoryMappedModel> {
        let model = self.models.get(model_id).ok_or(anyhow!("Model not cached"))?;

        let mut mmaps = Vec::new();

        for file in &model.files {
            // Memory-map large model files
            if file.path.extension() == Some("bin") || file.path.extension() == Some("safetensors") {
                let mmap = unsafe {
                    memmap2::MmapOptions::new()
                        .map(&std::fs::File::open(&file.path)?)?
                };
                mmaps.push(mmap);
            }
        }

        Ok(MemoryMappedModel {
            model_id: model_id.to_string(),
            mmaps,
        })
    }
}
```

**4.3 Deduplication (Save 50%+ storage)**
```rust
// Content-addressable storage for shared files
pub struct ContentAddressableStore {
    store_dir: PathBuf,
    index: HashMap<String, PathBuf>,  // hash -> path
}

impl ContentAddressableStore {
    fn insert(&mut self, hash: &str, file: &Path) -> Result<()> {
        let target = self.store_dir.join(hash);

        if !target.exists() {
            std::fs::copy(file, &target)?;
        }

        self.index.insert(hash.to_string(), target);
        Ok(())
    }

    fn get(&self, hash: &str) -> Option<&PathBuf> {
        self.index.get(hash)
    }
}
```

---

### 5. Resource Management

**Purpose:** Fair resource allocation and QoS

#### Architecture
```rust
pub struct ResourceManager {
    gpu_quotas: HashMap<String, GpuQuota>,
    memory_quotas: HashMap<String, MemoryQuota>,
    network_qos: NetworkQoS,
    priority_queue: PriorityQueue,
}

pub struct GpuQuota {
    user: String,
    max_gpus: u32,
    max_vram_gb: u64,
    priority: i32,
}

pub struct PriorityQueue {
    queues: BTreeMap<Priority, Vec<ContainerRequest>>,
}

pub enum Priority {
    Critical = 100,
    High = 75,
    Normal = 50,
    Low = 25,
    BestEffort = 0,
}
```

#### Features

**5.1 GPU Quotas**
```bash
# Set user quota
bolt quota set-gpu --user alice --max-gpus 4 --max-vram 64GB

# Container respects quota
bolt run --user alice --gpus 8 myimage  # ERROR: Exceeds quota
```

**5.2 Priority Scheduling**
```rust
impl PriorityQueue {
    async fn schedule_next(&mut self) -> Option<ContainerRequest> {
        // Highest priority first
        for (_, queue) in self.queues.iter_mut().rev() {
            if let Some(req) = queue.pop() {
                return Some(req);
            }
        }
        None
    }

    async fn preempt_if_needed(&mut self, new_request: ContainerRequest) -> Result<()> {
        if new_request.priority >= Priority::High {
            // Find lower priority container to evict
            let victim = self.find_preemptable_container(new_request.priority).await?;

            if let Some(victim_id) = victim {
                info!("Preempting container {} for higher priority request", victim_id);
                self.runtime.stop_container(&victim_id, 30).await?;
                self.requeue_container(&victim_id).await?;
            }
        }

        Ok(())
    }
}
```

---

## 📊 Performance Targets

| Metric | Docker | **Bolt Target** | Impact |
|--------|--------|-----------------|--------|
| Container startup | ~500ms | **<100ms** | 5x faster |
| GPU passthrough | ~100μs | **<1μs** | 100x faster |
| Model loading | ~60s | **<10s** | 6x faster |
| LLM tokens/sec | ~30 | **~60** | 2x faster |
| Multi-GPU efficiency | ~70% | **~95%** | 1.35x better |
| Memory overhead/container | ~50MB | **<10MB** | 5x more efficient |

---

## 🎯 Success Criteria

### MVP (v0.1.0)
- [x] Multi-GPU scheduling (4+ containers per GPU)
- [ ] Model serving (vLLM integration)
- [ ] Model caching (HuggingFace Hub)
- [ ] Docker CLI 95% compatible
- [ ] Benchmarks show 2-10x improvement

### Production (v1.0.0)
- [ ] Distributed training (32+ GPUs)
- [ ] MIG support (A100/H100)
- [ ] TensorRT optimization
- [ ] 99.9% uptime
- [ ] Fortune 500 deployments

---

*This architecture makes Bolt the definitive AI container runtime.*
