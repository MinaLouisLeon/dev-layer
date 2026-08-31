//! System telemetry: what the HUD exists to show.
//!
//! One background thread samples everything on a fixed tick and pushes a whole
//! [`MetricsSnapshot`] to the frontend as a single event. The frontend keeps
//! the history; Rust stays stateless apart from the rate deltas it must own.
//!
//! Sampling is deliberately tiered — a tool that reports your CPU usage must
//! not be a measurable part of it. Cheap counters (CPU, memory, GPU, network)
//! refresh every tick; expensive ones (the process table, disk list) refresh
//! every `slow_tick_every` ticks and are carried forward in between.

pub mod gpu;
mod sampler;

use serde::{Deserialize, Serialize};

pub use gpu::GpuMetrics;
pub use sampler::{host_info, spawn_sampler, MetricsStore, METRICS_EVENT};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsSnapshot {
    /// Unix milliseconds, so the frontend can detect gaps and stalls.
    pub timestamp_ms: u64,
    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    pub gpus: Vec<GpuMetrics>,
    pub network: NetworkMetrics,
    pub disks: Vec<DiskMetrics>,
    pub processes: Vec<ProcessMetrics>,
    pub uptime_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuMetrics {
    /// Whole-system usage, 0–100.
    pub usage: f32,
    /// Per logical core, 0–100. Length is stable for the life of the process.
    pub per_core: Vec<f32>,
    pub frequency_mhz: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryMetrics {
    pub used: u64,
    pub total: u64,
    pub swap_used: u64,
    pub swap_total: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkMetrics {
    /// Bytes per second, summed over every interface.
    pub rx_per_sec: u64,
    pub tx_per_sec: u64,
    pub rx_total: u64,
    pub tx_total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskMetrics {
    pub name: String,
    pub mount: String,
    pub used: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessMetrics {
    pub pid: u32,
    pub name: String,
    /// Normalized to whole-machine percent (divided by logical core count), so
    /// it matches what Task Manager shows rather than sysinfo's per-core value.
    pub cpu: f32,
    pub memory: u64,
}

/// Static facts about the machine, fetched once by the HUD rather than
/// re-sent on every tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostInfo {
    pub hostname: String,
    pub os: String,
    pub kernel: String,
    pub cpu_brand: String,
    pub physical_cores: usize,
    pub logical_cores: usize,
    pub total_memory: u64,
}
