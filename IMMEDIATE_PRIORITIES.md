# ⚡ Bolt Immediate Priorities
**Next 2-4 Weeks - Make Bolt Usable**

---

## 🎯 Week 1-2: Docker CLI Drop-in Replacement

### Priority 1: Core Docker Commands (CRITICAL)
**Goal:** `alias docker=bolt` works for basic workflows

#### Task 1.1: `bolt run` Docker Compatibility
**Current gap:** Missing many Docker flags

```rust
// File: src/cli/run.rs
// Add missing Docker flags:

#[derive(Parser)]
pub struct RunCommand {
    // ✅ Already have:
    image: String,
    command: Vec<String>,

    // ❌ Need to add:
    #[arg(long)]
    gpus: Option<String>,              // --gpus all, --gpus 2, --gpus device=0,1

    #[arg(short, long)]
    detach: bool,                      // -d, --detach

    #[arg(long)]
    rm: bool,                          // --rm (auto-remove)

    #[arg(short, long)]
    interactive: bool,                 // -i (keep STDIN open)

    #[arg(short, long)]
    tty: bool,                         // -t (allocate TTY)

    #[arg(short, long)]
    env: Vec<String>,                  // -e KEY=VALUE

    #[arg(short, long)]
    volume: Vec<String>,               // -v host:container

    #[arg(short, long)]
    publish: Vec<String>,              // -p 8080:80

    #[arg(long)]
    network: Option<String>,           // --network bridge

    #[arg(long)]
    cpus: Option<f32>,                 // --cpus 2.0

    #[arg(short, long)]
    memory: Option<String>,            // -m 2g

    #[arg(long)]
    entrypoint: Option<String>,        // --entrypoint /bin/sh

    #[arg(short, long)]
    workdir: Option<String>,           // -w /app

    #[arg(short, long)]
    user: Option<String>,              // -u 1000:1000

    #[arg(long)]
    privileged: bool,                  // --privileged

    #[arg(long)]
    restart: Option<String>,           // --restart always

    #[arg(long)]
    label: Vec<String>,                // --label key=value
}
```

**Implementation:**
```rust
// src/cli/run.rs

impl RunCommand {
    pub async fn execute(&self, runtime: &BoltRuntime) -> Result<()> {
        let mut container_config = ContainerConfig::default();

        // GPU support (Docker --gpus syntax)
        if let Some(ref gpus) = self.gpus {
            container_config.gpu = self.parse_gpus_flag(gpus)?;
        }

        // Environment variables
        for env in &self.env {
            if let Some((k, v)) = env.split_once('=') {
                container_config.env.insert(k.to_string(), v.to_string());
            }
        }

        // Volumes
        for vol in &self.volume {
            container_config.mounts.push(self.parse_volume(vol)?);
        }

        // Ports
        for port in &self.publish {
            container_config.ports.push(self.parse_port(port)?);
        }

        // Resource limits
        if let Some(cpus) = self.cpus {
            container_config.cpu_quota = Some((cpus * 100000.0) as u64);
        }
        if let Some(ref mem) = self.memory {
            container_config.memory_limit = Some(self.parse_memory(mem)?);
        }

        // Run container
        let container_id = runtime.create_container(&self.image, container_config).await?;
        runtime.start_container(&container_id).await?;

        // Detach or attach
        if self.detach {
            println!("{}", container_id);
        } else if self.interactive || self.tty {
            runtime.attach_container(&container_id).await?;
        }

        // Auto-remove
        if self.rm {
            runtime.remove_container(&container_id, true).await?;
        }

        Ok(())
    }

    fn parse_gpus_flag(&self, gpus: &str) -> Result<GpuConfig> {
        match gpus {
            "all" => Ok(GpuConfig::All),
            n if n.parse::<u32>().is_ok() => {
                Ok(GpuConfig::Count(n.parse().unwrap()))
            }
            devices if devices.starts_with("device=") => {
                let ids = devices.strip_prefix("device=").unwrap()
                    .split(',')
                    .map(String::from)
                    .collect();
                Ok(GpuConfig::Specific(ids))
            }
            _ => Err(anyhow!("Invalid --gpus format"))
        }
    }
}
```

**Files to create/modify:**
- ✅ Already exists: `src/cli/run.rs`
- 📝 Need to expand with Docker flags
- 📝 Add `parse_*` helper methods

**Time:** 2-3 days

---

#### Task 1.2: `bolt exec` - Interactive Shell
**Current gap:** No exec command

```rust
// File: src/cli/exec.rs (NEW FILE)

use clap::Parser;
use crate::Result;

#[derive(Parser)]
pub struct ExecCommand {
    /// Container name or ID
    container: String,

    /// Command to execute
    command: Vec<String>,

    #[arg(short, long)]
    interactive: bool,  // -i

    #[arg(short, long)]
    tty: bool,          // -t

    #[arg(short, long)]
    detach: bool,       // -d

    #[arg(short, long)]
    user: Option<String>,  // -u

    #[arg(short, long)]
    workdir: Option<String>,  // -w

    #[arg(short, long)]
    env: Vec<String>,  // -e
}

impl ExecCommand {
    pub async fn execute(&self, runtime: &BoltRuntime) -> Result<()> {
        // Find container
        let container = runtime.get_container(&self.container).await?;

        if !container.is_running() {
            return Err(anyhow!("Container {} is not running", self.container));
        }

        // Build exec config
        let exec_config = ExecConfig {
            command: self.command.clone(),
            interactive: self.interactive,
            tty: self.tty,
            user: self.user.clone(),
            workdir: self.workdir.clone(),
            env: self.env.clone(),
        };

        // Execute command in container
        if self.detach {
            let exec_id = runtime.exec_detached(&container.id, exec_config).await?;
            println!("{}", exec_id);
        } else {
            runtime.exec_interactive(&container.id, exec_config).await?;
        }

        Ok(())
    }
}
```

**Backend implementation:**
```rust
// File: src/runtime/exec.rs (NEW FILE)

use tokio::process::Command;
use std::os::unix::io::{AsRawFd, RawFd};

pub struct ExecConfig {
    pub command: Vec<String>,
    pub interactive: bool,
    pub tty: bool,
    pub user: Option<String>,
    pub workdir: Option<String>,
    pub env: Vec<String>,
}

impl BoltRuntime {
    pub async fn exec_interactive(&self, container_id: &str, config: ExecConfig) -> Result<()> {
        // Use nsenter to enter container namespaces
        let container_pid = self.get_container_pid(container_id).await?;

        let mut cmd = Command::new("nsenter");
        cmd.args([
            "-t", &container_pid.to_string(),
            "-m",  // mount namespace
            "-u",  // UTS namespace
            "-i",  // IPC namespace
            "-n",  // network namespace
            "-p",  // PID namespace
        ]);

        // User
        if let Some(ref user) = config.user {
            cmd.args(["--setuid", user]);
        }

        // Working directory
        if let Some(ref workdir) = config.workdir {
            cmd.current_dir(workdir);
        }

        // Environment
        for env in &config.env {
            cmd.env(env.split_once('=').unwrap().0, env.split_once('=').unwrap().1);
        }

        // Command
        cmd.args(&config.command);

        // Setup TTY
        if config.tty && config.interactive {
            self.setup_tty(&mut cmd).await?;
        }

        // Execute
        let status = cmd.status().await?;

        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }

        Ok(())
    }

    async fn setup_tty(&self, cmd: &mut Command) -> Result<()> {
        // Put terminal in raw mode
        use termios::{Termios, TCSANOW, ECHO, ICANON};

        let stdin_fd = std::io::stdin().as_raw_fd();
        let mut termios = Termios::from_fd(stdin_fd)?;

        // Disable canonical mode and echo
        termios.c_lflag &= !(ICANON | ECHO);
        termios::tcsetattr(stdin_fd, TCSANOW, &termios)?;

        // Forward signals
        cmd.stdin(std::process::Stdio::inherit());
        cmd.stdout(std::process::Stdio::inherit());
        cmd.stderr(std::process::Stdio::inherit());

        Ok(())
    }
}
```

**Dependencies to add:**
```toml
[dependencies]
nix = "0.27"  # For namespace operations
termios = "0.3"  # For TTY control
```

**Time:** 3-4 days

---

#### Task 1.3: `bolt logs` - Streaming Logs
**Current gap:** No logs command

```rust
// File: src/cli/logs.rs (NEW FILE)

#[derive(Parser)]
pub struct LogsCommand {
    /// Container name or ID
    container: String,

    #[arg(short, long)]
    follow: bool,  // -f (stream logs)

    #[arg(long)]
    tail: Option<usize>,  // --tail 100

    #[arg(long)]
    since: Option<String>,  // --since 2h

    #[arg(long)]
    timestamps: bool,  // -t
}

impl LogsCommand {
    pub async fn execute(&self, runtime: &BoltRuntime) -> Result<()> {
        if self.follow {
            runtime.stream_logs(&self.container, self.tail, self.timestamps).await?;
        } else {
            let logs = runtime.get_logs(&self.container, self.tail).await?;
            print!("{}", logs);
        }
        Ok(())
    }
}
```

**Backend:**
```rust
// src/runtime/logs.rs (NEW FILE)

impl BoltRuntime {
    pub async fn stream_logs(&self, container_id: &str, tail: Option<usize>, timestamps: bool) -> Result<()> {
        let log_path = self.get_container_log_path(container_id);

        // Use tail -f for streaming
        let mut cmd = tokio::process::Command::new("tail");
        cmd.arg("-f");
        if let Some(n) = tail {
            cmd.args(["-n", &n.to_string()]);
        }
        cmd.arg(&log_path);

        let mut child = cmd.stdout(Stdio::piped()).spawn()?;
        let stdout = child.stdout.take().unwrap();

        // Stream to stdout
        use tokio::io::{AsyncBufReadExt, BufReader};
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        while let Some(line) = lines.next_line().await? {
            if timestamps {
                let now = chrono::Utc::now();
                println!("{} {}", now.to_rfc3339(), line);
            } else {
                println!("{}", line);
            }
        }

        Ok(())
    }
}
```

**Time:** 1-2 days

---

### Priority 2: Multi-GPU Scheduler (CRITICAL FOR AI)

#### Task 2.1: GPU Allocation Manager
**File:** `src/runtime/gpu_scheduler.rs` (NEW FILE)

```rust
use std::collections::{HashMap, BTreeMap};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct GpuScheduler {
    /// GPU ID -> GPU state
    gpus: Arc<RwLock<HashMap<String, GpuState>>>,
    /// Container ID -> Allocated GPUs
    allocations: Arc<RwLock<HashMap<String, Vec<String>>>>,
    /// Scheduling strategy
    strategy: SchedulingStrategy,
}

#[derive(Debug, Clone)]
pub struct GpuState {
    pub id: String,
    pub name: String,
    pub total_memory_mb: u64,
    pub free_memory_mb: u64,
    pub utilization: f32,
    pub allocated_to: Vec<String>,  // Container IDs
    pub is_mig_enabled: bool,
    pub mig_instances: Vec<MigInstance>,
}

#[derive(Debug, Clone)]
pub struct MigInstance {
    pub id: String,
    pub gpu_slice: u32,
    pub memory_mb: u64,
    pub allocated_to: Option<String>,
}

pub enum SchedulingStrategy {
    RoundRobin,
    LeastUtilized,
    MostMemory,
    Exclusive,
}

impl GpuScheduler {
    pub async fn new() -> Result<Self> {
        // Detect all GPUs
        let gpus = Self::detect_gpus().await?;

        Ok(Self {
            gpus: Arc::new(RwLock::new(gpus)),
            allocations: Arc::new(RwLock::new(HashMap::new())),
            strategy: SchedulingStrategy::LeastUtilized,
        })
    }

    /// Allocate GPUs for a container
    pub async fn allocate(&self, container_id: &str, request: GpuRequest) -> Result<Vec<String>> {
        let mut gpus = self.gpus.write().await;
        let mut allocations = self.allocations.write().await;

        let selected_gpus = match request {
            GpuRequest::All => {
                // Allocate all available GPUs
                gpus.keys().cloned().collect()
            }
            GpuRequest::Count(n) => {
                // Allocate N GPUs using scheduling strategy
                self.select_gpus(&gpus, n).await?
            }
            GpuRequest::Specific(ids) => {
                // Allocate specific GPUs
                self.validate_gpu_ids(&gpus, &ids)?;
                ids
            }
            GpuRequest::Memory(memory_mb) => {
                // Allocate GPUs with enough memory
                self.select_gpus_by_memory(&gpus, memory_mb).await?
            }
        };

        // Mark GPUs as allocated
        for gpu_id in &selected_gpus {
            if let Some(gpu) = gpus.get_mut(gpu_id) {
                gpu.allocated_to.push(container_id.to_string());
                gpu.free_memory_mb = 0;  // Exclusive mode
            }
        }

        allocations.insert(container_id.to_string(), selected_gpus.clone());

        Ok(selected_gpus)
    }

    /// Deallocate GPUs when container stops
    pub async fn deallocate(&self, container_id: &str) -> Result<()> {
        let mut gpus = self.gpus.write().await;
        let mut allocations = self.allocations.write().await;

        if let Some(gpu_ids) = allocations.remove(container_id) {
            for gpu_id in gpu_ids {
                if let Some(gpu) = gpus.get_mut(&gpu_id) {
                    gpu.allocated_to.retain(|id| id != container_id);
                    gpu.free_memory_mb = gpu.total_memory_mb;
                }
            }
        }

        Ok(())
    }

    async fn select_gpus(&self, gpus: &HashMap<String, GpuState>, count: usize) -> Result<Vec<String>> {
        match self.strategy {
            SchedulingStrategy::RoundRobin => {
                // Simple round-robin
                let available: Vec<_> = gpus.iter()
                    .filter(|(_, state)| state.allocated_to.is_empty())
                    .take(count)
                    .map(|(id, _)| id.clone())
                    .collect();

                if available.len() < count {
                    return Err(anyhow!("Not enough free GPUs"));
                }

                Ok(available)
            }
            SchedulingStrategy::LeastUtilized => {
                // Sort by utilization
                let mut sorted: Vec<_> = gpus.iter()
                    .filter(|(_, state)| state.allocated_to.is_empty())
                    .collect();
                sorted.sort_by(|a, b| a.1.utilization.partial_cmp(&b.1.utilization).unwrap());

                Ok(sorted.into_iter().take(count).map(|(id, _)| id.clone()).collect())
            }
            _ => unimplemented!(),
        }
    }

    async fn detect_gpus() -> Result<HashMap<String, GpuState>> {
        // Use nvidia-smi to detect GPUs
        let output = tokio::process::Command::new("nvidia-smi")
            .args(["--query-gpu=index,name,memory.total", "--format=csv,noheader,nounits"])
            .output()
            .await?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut gpus = HashMap::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
            if parts.len() >= 3 {
                let id = format!("gpu:{}", parts[0]);
                let gpu = GpuState {
                    id: id.clone(),
                    name: parts[1].to_string(),
                    total_memory_mb: parts[2].parse().unwrap_or(0),
                    free_memory_mb: parts[2].parse().unwrap_or(0),
                    utilization: 0.0,
                    allocated_to: Vec::new(),
                    is_mig_enabled: false,
                    mig_instances: Vec::new(),
                };
                gpus.insert(id, gpu);
            }
        }

        Ok(gpus)
    }
}

#[derive(Debug, Clone)]
pub enum GpuRequest {
    All,
    Count(usize),
    Specific(Vec<String>),
    Memory(u64),  // Request by memory size
}
```

**Time:** 4-5 days

---

### Priority 3: Docker API Server

#### Task 3.1: Core API Endpoints
**File:** `src/docker_compat/api_server.rs`

**Already exists but needs expansion:**

```rust
// Add these critical endpoints:

// GET /containers/json - List containers
async fn list_containers(Query(params): Query<ListContainersParams>) -> Json<Vec<ContainerSummary>> {
    let runtime = get_runtime().await;
    let containers = runtime.list_containers(params.all).await?;
    Json(containers.into_iter().map(ContainerSummary::from).collect())
}

// POST /containers/create - Create container
async fn create_container(Json(config): Json<ContainerCreateRequest>) -> Json<ContainerCreateResponse> {
    let runtime = get_runtime().await;
    let id = runtime.create_container_from_docker_config(config).await?;
    Json(ContainerCreateResponse { id, warnings: vec![] })
}

// POST /containers/{id}/start - Start container
async fn start_container(Path(id): Path<String>) -> StatusCode {
    let runtime = get_runtime().await;
    runtime.start_container(&id).await?;
    StatusCode::NO_CONTENT
}

// POST /containers/{id}/stop - Stop container
async fn stop_container(Path(id): Path<String>, Query(params): Query<StopParams>) -> StatusCode {
    let runtime = get_runtime().await;
    runtime.stop_container(&id, params.t.unwrap_or(10)).await?;
    StatusCode::NO_CONTENT
}

// GET /containers/{id}/logs - Stream logs
async fn get_logs(
    Path(id): Path<String>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| stream_container_logs(socket, id))
}

async fn stream_container_logs(mut socket: WebSocket, container_id: String) {
    let runtime = get_runtime().await;
    let mut log_stream = runtime.get_log_stream(&container_id).await.unwrap();

    while let Some(line) = log_stream.next().await {
        if socket.send(Message::Text(line)).await.is_err() {
            break;
        }
    }
}
```

**Time:** 5-7 days

---

## 🎯 Week 3-4: AI Workload Foundations

### Priority 4: Model Serving Integration

#### Task 4.1: vLLM Integration
**Goal:** Fast LLM inference

```bash
# User workflow:
bolt serve \
  --model meta-llama/Llama-3-70B \
  --gpus 4 \
  --port 8000 \
  --max-batch-size 32
```

**Implementation:**
```rust
// src/ai/model_serving.rs (NEW FILE)

pub struct ModelServer {
    model_name: String,
    gpus: Vec<String>,
    port: u16,
    backend: ServingBackend,
}

pub enum ServingBackend {
    VLLM,
    TensorRT,
    ONNX,
}

impl ModelServer {
    pub async fn start(&self) -> Result<()> {
        match self.backend {
            ServingBackend::VLLM => self.start_vllm().await,
            _ => unimplemented!(),
        }
    }

    async fn start_vllm(&self) -> Result<()> {
        // Create container with vLLM
        let runtime = BoltRuntime::new()?;

        let container_config = ContainerConfig {
            image: "vllm/vllm-openai:latest".to_string(),
            gpus: GpuRequest::Specific(self.gpus.clone()),
            ports: vec![format!("{}:8000", self.port)],
            env: vec![
                format!("MODEL={}", self.model_name),
                "TENSOR_PARALLEL_SIZE=4".to_string(),
            ],
            ..Default::default()
        };

        runtime.create_and_start_container("vllm-server", container_config).await?;

        Ok(())
    }
}
```

**Time:** 3-4 days

---

## 📊 Summary: Next 4 Weeks

### Week 1
- [ ] Docker CLI compatibility (`run`, `exec`, `logs`)
- [ ] Fix compiler warnings
- [ ] Basic benchmarks

### Week 2
- [ ] Multi-GPU scheduler
- [ ] MIG support
- [ ] GPU memory tracking

### Week 3
- [ ] Docker API server (core endpoints)
- [ ] Model serving (vLLM)
- [ ] HuggingFace integration

### Week 4
- [ ] Performance benchmarks
- [ ] Documentation
- [ ] Integration tests

**After 4 weeks, Bolt will be:**
- ✅ Docker CLI compatible (90%)
- ✅ Multi-GPU scheduling working
- ✅ AI model serving ready
- ✅ 2-10x faster than Docker (proven)

---

*This roadmap focuses on making Bolt immediately usable for AI engineers who want a better Docker.*
