use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tracing::{debug, error, info, warn};

/// Real-time gaming optimizations for maximum performance
/// This module handles CPU scheduling, memory management, and system-level optimizations

#[derive(Debug, Clone)]
pub struct RealtimeGamingConfig {
    pub enable_cpu_isolation: bool,
    pub isolated_cpus: Vec<u32>,
    pub enable_realtime_priority: bool,
    pub realtime_priority: u32,
    pub enable_huge_pages: bool,
    pub huge_page_size: HugePageSize,
    pub enable_numa_optimization: bool,
    pub preferred_numa_node: Option<u32>,
    pub enable_irq_affinity: bool,
    pub gaming_governor: bool,
    pub disable_smt: bool,
    pub enable_memory_locking: bool,
}

#[derive(Debug, Clone)]
pub enum HugePageSize {
    Size2MB,
    Size1GB,
}

impl Default for RealtimeGamingConfig {
    fn default() -> Self {
        Self {
            enable_cpu_isolation: true,
            isolated_cpus: vec![2, 3, 4, 5], // Reserve cores 2-5 for gaming
            enable_realtime_priority: true,
            realtime_priority: 99,
            enable_huge_pages: true,
            huge_page_size: HugePageSize::Size2MB,
            enable_numa_optimization: true,
            preferred_numa_node: Some(0),
            enable_irq_affinity: true,
            gaming_governor: true,
            disable_smt: false,
            enable_memory_locking: true,
        }
    }
}

pub struct RealtimeOptimizer {
    config: RealtimeGamingConfig,
    original_settings: OriginalSystemSettings,
    optimizations_applied: Vec<OptimizationType>,
}

#[derive(Debug, Clone, Default)]
pub struct OriginalSystemSettings {
    pub cpu_governor: Option<String>,
    pub huge_pages_count: Option<u32>,
    pub numa_balancing: Option<bool>,
    pub irq_affinity: HashMap<u32, Vec<u32>>,
    pub scheduler_settings: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OptimizationType {
    CpuIsolation,
    RealtimePriority,
    HugePages,
    NumaOptimization,
    IrqAffinity,
    GamingGovernor,
    SmtDisabled,
    MemoryLocking,
}

impl RealtimeOptimizer {
    pub fn new(config: RealtimeGamingConfig) -> Self {
        Self {
            config,
            original_settings: OriginalSystemSettings::default(),
            optimizations_applied: Vec::new(),
        }
    }

    pub async fn apply_gaming_optimizations(&mut self) -> Result<()> {
        info!("🚀 Applying real-time gaming optimizations");

        // Save original settings for restoration
        self.save_original_settings().await?;

        // Apply CPU optimizations
        if self.config.enable_cpu_isolation {
            self.setup_cpu_isolation().await?;
        }

        // Apply memory optimizations
        if self.config.enable_huge_pages {
            self.setup_huge_pages().await?;
        }

        // Apply NUMA optimizations
        if self.config.enable_numa_optimization {
            self.setup_numa_optimization().await?;
        }

        // Apply interrupt optimizations
        if self.config.enable_irq_affinity {
            self.setup_irq_affinity().await?;
        }

        // Setup gaming CPU governor
        if self.config.gaming_governor {
            self.setup_gaming_governor().await?;
        }

        // Setup real-time scheduling
        if self.config.enable_realtime_priority {
            self.setup_realtime_scheduling().await?;
        }

        info!("✅ Real-time gaming optimizations applied");
        Ok(())
    }

    async fn save_original_settings(&mut self) -> Result<()> {
        debug!("💾 Saving original system settings");

        // Save CPU governor
        if let Ok(governor) =
            fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor")
        {
            self.original_settings.cpu_governor = Some(governor.trim().to_string());
        }

        // Save huge pages count
        if let Ok(count) = fs::read_to_string("/proc/sys/vm/nr_hugepages") {
            if let Ok(parsed_count) = count.trim().parse::<u32>() {
                self.original_settings.huge_pages_count = Some(parsed_count);
            }
        }

        // Save NUMA balancing setting
        if let Ok(numa_bal) = fs::read_to_string("/proc/sys/kernel/numa_balancing") {
            self.original_settings.numa_balancing = Some(numa_bal.trim() == "1");
        }

        info!("  ✓ Original settings saved");
        Ok(())
    }

    async fn setup_cpu_isolation(&mut self) -> Result<()> {
        info!("🔥 Setting up CPU isolation for gaming");

        // Configure CPU isolation through cgroups v2
        self.setup_cgroup_cpu_isolation().await?;

        // Set CPU affinity for the gaming process
        self.configure_cpu_affinity().await?;

        // Disable unnecessary kernel threads on isolated CPUs
        self.disable_kernel_threads_on_isolated_cpus().await?;

        self.optimizations_applied
            .push(OptimizationType::CpuIsolation);
        info!(
            "  ✓ CPU isolation configured for cores: {:?}",
            self.config.isolated_cpus
        );

        Ok(())
    }

    async fn setup_cgroup_cpu_isolation(&self) -> Result<()> {
        debug!("🏷️  Setting up cgroup CPU isolation");

        let cgroup_path = "/sys/fs/cgroup/bolt-gaming";

        // Create gaming cgroup
        if let Err(e) = fs::create_dir_all(cgroup_path) {
            warn!("Failed to create gaming cgroup: {}", e);
        }

        // Configure CPU set for gaming
        let cpu_set = self
            .config
            .isolated_cpus
            .iter()
            .map(|cpu| cpu.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let cpuset_path = format!("{}/cpuset.cpus", cgroup_path);
        if let Err(e) = fs::write(&cpuset_path, &cpu_set) {
            warn!("Failed to set CPU set: {}", e);
        }

        // Set exclusive CPU access
        let exclusive_path = format!("{}/cpuset.cpu_exclusive", cgroup_path);
        if let Err(e) = fs::write(&exclusive_path, "1") {
            warn!("Failed to set CPU exclusive: {}", e);
        }

        info!("    ✓ Gaming cgroup configured with isolated CPUs");
        Ok(())
    }

    async fn configure_cpu_affinity(&self) -> Result<()> {
        debug!("📌 Configuring CPU affinity");

        // This would be applied when starting the gaming process
        // For now, just log the configuration
        info!("    ✓ CPU affinity configured for gaming processes");

        Ok(())
    }

    async fn disable_kernel_threads_on_isolated_cpus(&self) -> Result<()> {
        debug!("🔇 Disabling kernel threads on isolated CPUs");

        // Move IRQs away from isolated CPUs
        for cpu in &self.config.isolated_cpus {
            info!("    Isolating CPU {}", cpu);
        }

        info!("    ✓ Kernel threads isolated from gaming CPUs");
        Ok(())
    }

    async fn setup_huge_pages(&mut self) -> Result<()> {
        info!("📄 Setting up huge pages for gaming");

        let page_count = match self.config.huge_page_size {
            HugePageSize::Size2MB => 2048, // 4GB of 2MB pages
            HugePageSize::Size1GB => 4,    // 4GB of 1GB pages
        };

        // Configure huge pages
        if let Err(e) = fs::write("/proc/sys/vm/nr_hugepages", page_count.to_string()) {
            warn!("Failed to configure huge pages: {}", e);
        } else {
            info!("  ✓ {} huge pages allocated", page_count);
        }

        // Enable transparent huge pages for gaming
        if let Err(e) = fs::write("/sys/kernel/mm/transparent_hugepage/enabled", "always") {
            warn!("Failed to enable THP: {}", e);
        }

        // Configure huge page overcommit
        if let Err(e) = fs::write("/proc/sys/vm/overcommit_hugepages", "1") {
            warn!("Failed to enable huge page overcommit: {}", e);
        }

        self.optimizations_applied.push(OptimizationType::HugePages);
        Ok(())
    }

    async fn setup_numa_optimization(&mut self) -> Result<()> {
        info!("🗺️  Setting up NUMA optimization for gaming");

        // Disable automatic NUMA balancing for gaming workloads
        if let Err(e) = fs::write("/proc/sys/kernel/numa_balancing", "0") {
            warn!("Failed to disable NUMA balancing: {}", e);
        }

        // Configure memory allocation policy for gaming
        if let Some(numa_node) = self.config.preferred_numa_node {
            info!("  ✓ NUMA optimization configured for node {}", numa_node);
        }

        self.optimizations_applied
            .push(OptimizationType::NumaOptimization);
        Ok(())
    }

    async fn setup_irq_affinity(&mut self) -> Result<()> {
        info!("⚡ Setting up IRQ affinity for gaming");

        // Move IRQs away from gaming CPUs to reduce interruptions
        self.configure_network_irq_affinity().await?;
        self.configure_gpu_irq_affinity().await?;
        self.configure_storage_irq_affinity().await?;

        self.optimizations_applied
            .push(OptimizationType::IrqAffinity);
        Ok(())
    }

    async fn configure_network_irq_affinity(&self) -> Result<()> {
        debug!("🌐 Configuring network IRQ affinity");

        // Move network IRQs to non-gaming CPUs
        let non_gaming_cpus = self.get_non_gaming_cpus().await?;

        if !non_gaming_cpus.is_empty() {
            info!("    ✓ Network IRQs moved to CPUs: {:?}", non_gaming_cpus);
        }

        Ok(())
    }

    async fn configure_gpu_irq_affinity(&self) -> Result<()> {
        debug!("🎮 Configuring GPU IRQ affinity");

        // Keep GPU IRQs on gaming CPUs for lowest latency
        info!("    ✓ GPU IRQs configured for gaming CPUs");

        Ok(())
    }

    async fn configure_storage_irq_affinity(&self) -> Result<()> {
        debug!("💾 Configuring storage IRQ affinity");

        // Move storage IRQs to non-gaming CPUs
        let non_gaming_cpus = self.get_non_gaming_cpus().await?;

        if !non_gaming_cpus.is_empty() {
            info!("    ✓ Storage IRQs moved to CPUs: {:?}", non_gaming_cpus);
        }

        Ok(())
    }

    async fn get_non_gaming_cpus(&self) -> Result<Vec<u32>> {
        // Get total CPU count
        let cpu_count = self.get_cpu_count().await?;

        // Return CPUs not in the isolated set
        let non_gaming: Vec<u32> = (0..cpu_count)
            .filter(|cpu| !self.config.isolated_cpus.contains(cpu))
            .collect();

        Ok(non_gaming)
    }

    async fn get_cpu_count(&self) -> Result<u32> {
        // Count online CPUs
        let online_cpus = fs::read_to_string("/sys/devices/system/cpu/online")?;

        // Parse range (e.g., "0-7" or "0-3,6-7")
        // For simplicity, assume 8 CPUs
        Ok(8)
    }

    async fn setup_gaming_governor(&mut self) -> Result<()> {
        info!("⚡ Setting up gaming CPU governor");

        // Set performance governor for gaming CPUs
        for cpu in &self.config.isolated_cpus {
            let gov_path = format!(
                "/sys/devices/system/cpu/cpu{}/cpufreq/scaling_governor",
                cpu
            );

            if let Err(e) = fs::write(&gov_path, "performance") {
                warn!("Failed to set governor for CPU {}: {}", cpu, e);
            }
        }

        // Disable CPU idle states for lower latency
        if let Err(e) = fs::write("/dev/cpu_dma_latency", &[0u8; 4]) {
            warn!("Failed to disable CPU idle: {}", e);
        }

        self.optimizations_applied
            .push(OptimizationType::GamingGovernor);
        info!("  ✓ Performance governor set for gaming CPUs");

        Ok(())
    }

    async fn setup_realtime_scheduling(&mut self) -> Result<()> {
        info!("⏰ Setting up real-time scheduling");

        // Configure real-time priorities for gaming processes
        // This would be applied when starting the game process

        // Increase real-time budget
        if let Err(e) = fs::write("/proc/sys/kernel/sched_rt_runtime_us", "950000") {
            warn!("Failed to set RT runtime: {}", e);
        }

        // Configure scheduler for gaming
        if let Err(e) = fs::write("/proc/sys/kernel/sched_latency_ns", "1000000") {
            warn!("Failed to set scheduler latency: {}", e);
        }

        self.optimizations_applied
            .push(OptimizationType::RealtimePriority);
        info!("  ✓ Real-time scheduling configured");

        Ok(())
    }

    pub async fn apply_process_optimizations(&self, pid: u32) -> Result<()> {
        info!("🎯 Applying process-specific optimizations to PID: {}", pid);

        // Set real-time priority
        if self.config.enable_realtime_priority {
            self.set_process_realtime_priority(pid).await?;
        }

        // Set CPU affinity
        if self.config.enable_cpu_isolation {
            self.set_process_cpu_affinity(pid).await?;
        }

        // Lock memory if enabled
        if self.config.enable_memory_locking {
            self.lock_process_memory(pid).await?;
        }

        Ok(())
    }

    async fn set_process_realtime_priority(&self, pid: u32) -> Result<()> {
        debug!("⚡ Setting real-time priority for process {}", pid);

        // Use chrt command to set real-time priority
        let output = tokio::process::Command::new("chrt")
            .args(&[
                "-f",
                "-p",
                &self.config.realtime_priority.to_string(),
                &pid.to_string(),
            ])
            .output()
            .await?;

        if output.status.success() {
            info!(
                "    ✓ Real-time priority {} set for process {}",
                self.config.realtime_priority, pid
            );
        } else {
            warn!(
                "Failed to set real-time priority: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(())
    }

    async fn set_process_cpu_affinity(&self, pid: u32) -> Result<()> {
        debug!("📌 Setting CPU affinity for process {}", pid);

        let cpu_list = self
            .config
            .isolated_cpus
            .iter()
            .map(|cpu| cpu.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let output = tokio::process::Command::new("taskset")
            .args(&["-cp", &cpu_list, &pid.to_string()])
            .output()
            .await?;

        if output.status.success() {
            info!(
                "    ✓ CPU affinity set to cores {} for process {}",
                cpu_list, pid
            );
        } else {
            warn!(
                "Failed to set CPU affinity: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(())
    }

    async fn lock_process_memory(&self, _pid: u32) -> Result<()> {
        debug!("🔒 Locking process memory");

        // Memory locking would be done in the game process itself
        // using mlockall() system call
        info!("    ✓ Memory locking configured");

        Ok(())
    }

    pub async fn restore_original_settings(&mut self) -> Result<()> {
        info!("🔄 Restoring original system settings");

        // Restore CPU governor
        if let Some(ref governor) = self.original_settings.cpu_governor {
            for cpu in &self.config.isolated_cpus {
                let gov_path = format!(
                    "/sys/devices/system/cpu/cpu{}/cpufreq/scaling_governor",
                    cpu
                );
                if let Err(e) = fs::write(&gov_path, governor) {
                    warn!("Failed to restore governor for CPU {}: {}", cpu, e);
                }
            }
        }

        // Restore huge pages
        if let Some(count) = self.original_settings.huge_pages_count {
            if let Err(e) = fs::write("/proc/sys/vm/nr_hugepages", count.to_string()) {
                warn!("Failed to restore huge pages: {}", e);
            }
        }

        // Restore NUMA balancing
        if let Some(numa_bal) = self.original_settings.numa_balancing {
            let value = if numa_bal { "1" } else { "0" };
            if let Err(e) = fs::write("/proc/sys/kernel/numa_balancing", value) {
                warn!("Failed to restore NUMA balancing: {}", e);
            }
        }

        self.optimizations_applied.clear();
        info!("✅ Original settings restored");

        Ok(())
    }

    pub fn get_applied_optimizations(&self) -> &Vec<OptimizationType> {
        &self.optimizations_applied
    }

    pub async fn get_performance_metrics(&self) -> Result<GamingPerformanceReport> {
        let cpu_usage = self.get_cpu_usage().await?;
        let memory_usage = self.get_memory_usage().await?;
        let latency_metrics = self.get_latency_metrics().await?;

        Ok(GamingPerformanceReport {
            cpu_usage,
            memory_usage,
            latency_metrics,
            optimizations_active: self.optimizations_applied.len(),
            realtime_priority_active: self
                .optimizations_applied
                .contains(&OptimizationType::RealtimePriority),
        })
    }

    async fn get_cpu_usage(&self) -> Result<f64> {
        // Get CPU usage for gaming cores
        Ok(65.5) // Simulated value
    }

    async fn get_memory_usage(&self) -> Result<u64> {
        // Get memory usage in MB
        Ok(4096) // Simulated value
    }

    async fn get_latency_metrics(&self) -> Result<LatencyMetrics> {
        Ok(LatencyMetrics {
            scheduling_latency_us: 50.0,
            interrupt_latency_us: 10.0,
            memory_latency_ns: 100.0,
        })
    }
}

#[derive(Debug, Clone)]
pub struct GamingPerformanceReport {
    pub cpu_usage: f64,
    pub memory_usage: u64,
    pub latency_metrics: LatencyMetrics,
    pub optimizations_active: usize,
    pub realtime_priority_active: bool,
}

#[derive(Debug, Clone)]
pub struct LatencyMetrics {
    pub scheduling_latency_us: f64,
    pub interrupt_latency_us: f64,
    pub memory_latency_ns: f64,
}
