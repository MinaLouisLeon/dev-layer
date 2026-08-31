//! The sampling thread.

use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use parking_lot::RwLock;
use sysinfo::{
    CpuRefreshKind, Disks, MemoryRefreshKind, Networks, ProcessRefreshKind, ProcessesToUpdate,
    RefreshKind, System,
};
use tauri::{AppHandle, Emitter, Manager};

use crate::config::MetricsConfig;
use crate::error::{Error, Result};
use crate::metrics::gpu::GpuSampler;
use crate::metrics::{
    CpuMetrics, DiskMetrics, HostInfo, MemoryMetrics, MetricsSnapshot, NetworkMetrics,
    ProcessMetrics,
};
use crate::AppState;

pub const METRICS_EVENT: &str = "metrics::tick";

/// Keeps the most recent snapshot so a HUD window created mid-session (a
/// monitor was just plugged in) renders immediately instead of showing empty
/// gauges until the next tick.
#[derive(Default)]
pub struct MetricsStore {
    latest: RwLock<Option<MetricsSnapshot>>,
}

impl MetricsStore {
    pub fn latest(&self) -> Option<MetricsSnapshot> {
        self.latest.read().clone()
    }

    fn set(&self, snapshot: MetricsSnapshot) {
        *self.latest.write() = Some(snapshot);
    }
}

pub fn spawn_sampler(app: AppHandle, config: MetricsConfig) -> Result<()> {
    // Dropping this sender at teardown wakes the thread immediately, so
    // shutdown never waits out a full tick.
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    crate::safety::register("metrics sampler", move || drop(stop_tx));

    std::thread::Builder::new()
        .name("dev-layer/metrics".into())
        .spawn(move || {
            let interval = Duration::from_millis(config.interval_ms.max(200));
            let mut collector = Collector::new(&config);
            let mut tick: u64 = 0;

            loop {
                match stop_rx.recv_timeout(interval) {
                    Err(RecvTimeoutError::Timeout) => {}
                    // Sender dropped (teardown) or signalled: stop.
                    _ => break,
                }

                let snapshot = collector.sample(tick);
                tick = tick.wrapping_add(1);

                let state = app.state::<AppState>();
                state.metrics.set(snapshot.clone());
                if let Err(e) = app.emit(METRICS_EVENT, &snapshot) {
                    tracing::warn!(error = %e, "could not emit metrics");
                }
            }
            tracing::debug!("metrics sampler stopped");
        })
        .map_err(|e| Error::Platform(e.to_string()))?;

    Ok(())
}

struct Collector {
    system: System,
    networks: Networks,
    disks: Disks,
    gpu: GpuSampler,
    logical_cores: f32,
    top_processes: usize,
    slow_tick_every: u64,
    last_sample: Instant,
    /// Carried between slow ticks.
    processes: Vec<ProcessMetrics>,
    disk_list: Vec<DiskMetrics>,
}

impl Collector {
    fn new(config: &MetricsConfig) -> Self {
        let system = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::nothing().with_cpu_usage().with_frequency())
                .with_memory(MemoryRefreshKind::everything()),
        );
        let logical_cores = system.cpus().len().max(1) as f32;

        Self {
            system,
            networks: Networks::new_with_refreshed_list(),
            disks: Disks::new_with_refreshed_list(),
            gpu: GpuSampler::new(),
            logical_cores,
            top_processes: config.top_processes,
            slow_tick_every: config.slow_tick_every.max(1) as u64,
            last_sample: Instant::now(),
            processes: Vec::new(),
            disk_list: Vec::new(),
        }
    }

    fn sample(&mut self, tick: u64) -> MetricsSnapshot {
        let elapsed = self.last_sample.elapsed().as_secs_f64().max(0.001);
        self.last_sample = Instant::now();

        self.system.refresh_cpu_usage();
        self.system.refresh_memory();

        let cpu = CpuMetrics {
            usage: self.system.global_cpu_usage(),
            per_core: self.system.cpus().iter().map(|c| c.cpu_usage()).collect(),
            frequency_mhz: self
                .system
                .cpus()
                .first()
                .map(|c| c.frequency())
                .unwrap_or(0),
        };

        let memory = MemoryMetrics {
            used: self.system.used_memory(),
            total: self.system.total_memory(),
            swap_used: self.system.used_swap(),
            swap_total: self.system.total_swap(),
        };

        // sysinfo reports bytes since the previous refresh, so the rate is
        // only correct if we divide by the *actual* elapsed time — the tick
        // interval is a target, not a guarantee.
        self.networks.refresh(false);
        let network = self
            .networks
            .iter()
            .fold(NetworkMetrics::default(), |mut acc, (_, data)| {
                acc.rx_per_sec += (data.received() as f64 / elapsed) as u64;
                acc.tx_per_sec += (data.transmitted() as f64 / elapsed) as u64;
                acc.rx_total += data.total_received();
                acc.tx_total += data.total_transmitted();
                acc
            });

        if tick % self.slow_tick_every == 0 {
            self.refresh_slow();
        }

        MetricsSnapshot {
            timestamp_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            cpu,
            memory,
            gpus: self.gpu.sample(),
            network,
            disks: self.disk_list.clone(),
            processes: self.processes.clone(),
            uptime_secs: System::uptime(),
        }
    }

    /// The expensive half: enumerating processes and disks.
    fn refresh_slow(&mut self) {
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );

        let mut processes: Vec<ProcessMetrics> = self
            .system
            .processes()
            .iter()
            .map(|(pid, process)| ProcessMetrics {
                pid: pid.as_u32(),
                name: process.name().to_string_lossy().to_string(),
                cpu: process.cpu_usage() / self.logical_cores,
                memory: process.memory(),
            })
            .collect();

        processes.sort_by(|a, b| b.cpu.total_cmp(&a.cpu));
        processes.truncate(self.top_processes);
        self.processes = processes;

        self.disks.refresh(true);
        self.disk_list = self
            .disks
            .iter()
            .map(|disk| DiskMetrics {
                name: disk.name().to_string_lossy().to_string(),
                mount: disk.mount_point().to_string_lossy().to_string(),
                used: disk.total_space().saturating_sub(disk.available_space()),
                total: disk.total_space(),
            })
            .collect();
    }
}

/// Static machine facts, read on demand rather than sent on every tick.
pub fn host_info() -> HostInfo {
    let system = System::new_with_specifics(
        RefreshKind::nothing()
            .with_cpu(CpuRefreshKind::nothing())
            .with_memory(MemoryRefreshKind::everything()),
    );

    HostInfo {
        hostname: System::host_name().unwrap_or_else(|| "unknown".into()),
        os: System::long_os_version().unwrap_or_else(|| "unknown".into()),
        kernel: System::kernel_version().unwrap_or_else(|| "unknown".into()),
        cpu_brand: system
            .cpus()
            .first()
            .map(|c| c.brand().trim().to_string())
            .unwrap_or_default(),
        physical_cores: System::physical_core_count().unwrap_or(0),
        logical_cores: system.cpus().len(),
        total_memory: system.total_memory(),
    }
}
