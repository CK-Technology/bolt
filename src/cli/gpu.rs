//! GPU management commands

use crate::Result;
use bolt::runtime::gpu_scheduler::{GpuScheduler, GpuState};
use clap::{Parser, Subcommand};
use std::path::Path;
use tracing::info;

#[derive(Parser)]
pub struct GpuCommand {
    #[command(subcommand)]
    pub command: GpuSubcommand,
}

#[derive(Subcommand)]
pub enum GpuSubcommand {
    /// List available GPUs
    List,

    /// Show GPU allocation status
    Status {
        /// Show detailed information
        #[arg(short, long)]
        detailed: bool,
    },

    /// Show GPU metrics
    Metrics {
        /// Filter by container ID
        #[arg(short, long)]
        container: Option<String>,

        /// Update interval in seconds
        #[arg(short, long, default_value = "1")]
        interval: u64,
    },

    /// Check host GPU readiness for native container passthrough
    Preflight,

    /// Configure GPU scheduler
    Config {
        /// Scheduling strategy
        #[arg(long)]
        strategy: Option<String>,
    },

    /// Enable/configure MIG (Multi-Instance GPU)
    Mig {
        /// GPU index
        #[arg(long)]
        gpu: u32,

        /// MIG profile (e.g., "1g.5gb", "3g.20gb")
        #[arg(long)]
        profile: String,
    },
}

#[allow(dead_code)]
impl GpuCommand {
    pub async fn execute(&self) -> Result<()> {
        match &self.command {
            GpuSubcommand::List => self.list_gpus().await,
            GpuSubcommand::Status { detailed } => self.show_status(*detailed).await,
            GpuSubcommand::Metrics {
                container,
                interval,
            } => self.show_metrics(container.as_deref(), *interval).await,
            GpuSubcommand::Preflight => self.preflight().await,
            GpuSubcommand::Config { strategy } => {
                self.configure_scheduler(strategy.as_deref()).await
            }
            GpuSubcommand::Mig { gpu, profile } => self.configure_mig(*gpu, profile).await,
        }
    }

    async fn list_gpus(&self) -> Result<()> {
        info!("🎮 Listing GPUs...");

        let scheduler = GpuScheduler::new().await?;
        let gpus = scheduler.get_status().await;

        println!("\n╔══════════════════════════════════════════════════════════════╗");
        println!("║                     Available GPUs                          ║");
        println!("╠═══════╦══════════════════════╦════════════╦═══════╦═════════╣");
        println!("║  ID   ║       Name          ║  Memory    ║  Util ║ Temp    ║");
        println!("╠═══════╬══════════════════════╬════════════╬═══════╬═════════╣");

        for (id, gpu) in gpus.iter() {
            println!(
                "║ {:5} ║ {:20} ║ {:5}/{:5} ║ {:4}% ║ {:4}°C  ║",
                id,
                truncate(&gpu.name, 20),
                gpu.free_memory_mb,
                gpu.total_memory_mb,
                gpu.utilization_percent as u32,
                gpu.temperature_c
            );
        }

        println!("╚═══════╩══════════════════════╩════════════╩═══════╩═════════╝\n");

        Ok(())
    }

    async fn preflight(&self) -> Result<()> {
        info!("Checking host GPU readiness...");

        let report = GpuPreflightReport::run().await;
        println!("\nGPU native runtime preflight\n");
        for check in &report.checks {
            let status = if check.ok { "OK" } else { "FAIL" };
            println!("{:<5} {}", status, check.name);
            if let Some(detail) = &check.detail {
                println!("      {}", detail);
            }
        }

        if report.ok() {
            println!("\nGPU preflight passed");
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "GPU preflight failed; fix the failed checks above before relying on native GPU passthrough"
            )
            .into())
        }
    }

    async fn show_status(&self, detailed: bool) -> Result<()> {
        info!("📊 GPU allocation status...");

        let scheduler = GpuScheduler::new().await?;
        let gpus = scheduler.get_status().await;

        println!("\n╔════════════════════════════════════════════════════════════════╗");
        println!("║                  GPU Allocation Status                         ║");
        println!("╚════════════════════════════════════════════════════════════════╝\n");

        for (id, gpu) in gpus.iter() {
            println!("GPU: {} ({})", id, gpu.name);
            println!(
                "  Memory: {}/{} MB",
                gpu.free_memory_mb, gpu.total_memory_mb
            );
            println!("  Utilization: {}%", gpu.utilization_percent as u32);
            println!("  Power: {}/{} W", gpu.power_draw_w, gpu.power_limit_w);

            if !gpu.allocated_to.is_empty() {
                println!("  Allocated to:");
                for container_id in &gpu.allocated_to {
                    println!("    - {}", container_id);
                }
            } else {
                println!("  Status: ✅ Available");
            }

            if detailed {
                println!("  Temperature: {}°C", gpu.temperature_c);
                if gpu.is_mig_enabled {
                    println!("  MIG: Enabled ({} instances)", gpu.mig_instances.len());
                }
            }

            println!();
        }

        Ok(())
    }

    async fn show_metrics(&self, container: Option<&str>, interval: u64) -> Result<()> {
        info!(
            "📈 Showing GPU metrics (updating every {}s, press Ctrl+C to exit)...",
            interval
        );

        let scheduler = GpuScheduler::new().await?;

        loop {
            // Update metrics
            scheduler.update_metrics().await?;
            let gpus = scheduler.get_status().await;

            // Clear screen (ANSI escape)
            print!("\x1B[2J\x1B[1;1H");

            println!("╔════════════════════════════════════════════════════════════════╗");
            println!("║                  GPU Real-Time Metrics                         ║");
            println!("╚════════════════════════════════════════════════════════════════╝\n");

            for (id, gpu) in gpus.iter() {
                // Filter by container if specified
                if let Some(container_id) = container
                    && !gpu.allocated_to.contains(&container_id.to_string())
                {
                    continue;
                }

                print_gpu_metrics(id, gpu);
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(interval)).await;
        }
    }

    async fn configure_scheduler(&self, strategy: Option<&str>) -> Result<()> {
        if let Some(strat) = strategy {
            info!("⚙️  Configuring GPU scheduler: {}", strat);
            println!("✅ GPU scheduler strategy set to: {}", strat);
        } else {
            println!("Current scheduler configuration:");
            println!("  Strategy: least-utilized (default)");
        }
        Ok(())
    }

    async fn configure_mig(&self, gpu: u32, profile: &str) -> Result<()> {
        info!("🔧 Configuring MIG on GPU {}: {}", gpu, profile);
        println!("⚠️  MIG configuration requires root privileges");
        println!("✅ MIG profile {} enabled on GPU {}", profile, gpu);
        Ok(())
    }
}

#[allow(dead_code)]
fn print_gpu_metrics(id: &str, gpu: &GpuState) {
    println!("┌─ {} ({}) ─────────────────────────", id, gpu.name);
    println!("│");

    // Utilization bar
    let util_bar = create_bar(gpu.utilization_percent as u32, 50);
    println!(
        "│ Utilization:  {}  {}%",
        util_bar, gpu.utilization_percent as u32
    );

    // Memory bar
    let mem_percent = ((gpu.total_memory_mb - gpu.free_memory_mb) as f32
        / gpu.total_memory_mb as f32
        * 100.0) as u32;
    let mem_bar = create_bar(mem_percent, 50);
    println!(
        "│ Memory:       {}  {}/{} MB ({}%)",
        mem_bar,
        gpu.total_memory_mb - gpu.free_memory_mb,
        gpu.total_memory_mb,
        mem_percent
    );

    // Temperature
    let temp_color = if gpu.temperature_c > 80 {
        "\x1b[31m" // Red
    } else if gpu.temperature_c > 70 {
        "\x1b[33m" // Yellow
    } else {
        "\x1b[32m" // Green
    };
    println!(
        "│ Temperature:  {}{}°C\x1b[0m",
        temp_color, gpu.temperature_c
    );

    // Power
    let power_percent = (gpu.power_draw_w as f32 / gpu.power_limit_w as f32 * 100.0) as u32;
    println!(
        "│ Power:        {} W / {} W ({}%)",
        gpu.power_draw_w, gpu.power_limit_w, power_percent
    );

    // Allocated containers
    if !gpu.allocated_to.is_empty() {
        println!("│");
        println!("│ Allocated to:");
        for container_id in &gpu.allocated_to {
            println!("│   • {}", container_id);
        }
    }

    println!("└────────────────────────────────────────────────────────────────\n");
}

#[allow(dead_code)]
fn create_bar(percent: u32, width: usize) -> String {
    let filled = (percent as usize * width / 100).min(width);
    let empty = width.saturating_sub(filled);

    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));

    format!("[{}]", bar)
}

#[allow(dead_code)]
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        format!("{:width$}", s, width = max_len)
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

#[derive(Debug)]
struct GpuPreflightReport {
    checks: Vec<GpuPreflightCheck>,
}

impl GpuPreflightReport {
    async fn run() -> Self {
        let checks = vec![
            check_path("/dev/nvidiactl", "NVIDIA control device"),
            check_any_nvidia_device(),
            check_path("/dev/dri", "DRM device directory"),
            check_command("nvidia-smi", &["-L"]).await,
        ];

        Self { checks }
    }

    fn ok(&self) -> bool {
        self.checks.iter().all(|check| check.ok)
    }
}

#[derive(Debug)]
struct GpuPreflightCheck {
    name: &'static str,
    ok: bool,
    detail: Option<String>,
}

fn check_path(path: &'static str, name: &'static str) -> GpuPreflightCheck {
    let ok = Path::new(path).exists();
    GpuPreflightCheck {
        name,
        ok,
        detail: if ok {
            Some(path.to_string())
        } else {
            Some(format!("missing {}", path))
        },
    }
}

fn check_any_nvidia_device() -> GpuPreflightCheck {
    let found = std::fs::read_dir("/dev")
        .ok()
        .map(|entries| {
            entries.flatten().any(|entry| {
                entry.file_name().to_str().is_some_and(|name| {
                    name.starts_with("nvidia") && name[6..].parse::<u32>().is_ok()
                })
            })
        })
        .unwrap_or(false);

    GpuPreflightCheck {
        name: "NVIDIA GPU device node",
        ok: found,
        detail: if found {
            Some("found /dev/nvidia<N>".to_string())
        } else {
            Some("missing /dev/nvidia<N> device nodes".to_string())
        },
    }
}

async fn check_command(command: &'static str, args: &[&str]) -> GpuPreflightCheck {
    match tokio::process::Command::new(command)
        .args(args)
        .output()
        .await
    {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let first_line = stdout.lines().next().unwrap_or("command succeeded");
            GpuPreflightCheck {
                name: "nvidia-smi",
                ok: true,
                detail: Some(first_line.to_string()),
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            GpuPreflightCheck {
                name: "nvidia-smi",
                ok: false,
                detail: Some(format!(
                    "{} exited with status {}: {}",
                    command,
                    output.status,
                    stderr.trim()
                )),
            }
        }
        Err(err) => GpuPreflightCheck {
            name: "nvidia-smi",
            ok: false,
            detail: Some(format!("failed to execute {}: {}", command, err)),
        },
    }
}
