//! GPU telemetry.
//!
//! There is no single Windows API that reports GPU load for every vendor, so
//! this is a two-backend affair:
//!
//! * **NVML** (NVIDIA only) — accurate utilization, VRAM totals, temperature.
//! * **PDH performance counters** — vendor-neutral fallback, the same source
//!   Task Manager reads. No temperature, and no VRAM capacity.
//!
//! Anything unavailable is reported as `None` rather than zero: a HUD that
//! confidently shows "GPU 0 %" on a machine it cannot measure is worse than
//! one that says it does not know.

#[cfg(all(windows, feature = "nvidia"))]
mod nvml;
#[cfg(windows)]
mod pdh;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuMetrics {
    pub name: String,
    /// Percent, 0–100.
    pub utilization: Option<f32>,
    pub memory_used: Option<u64>,
    pub memory_total: Option<u64>,
    pub temperature_c: Option<u32>,
    /// Which backend produced this reading, surfaced in the HUD so the numbers
    /// are interpretable (PDH cannot report temperature or VRAM capacity).
    pub source: GpuSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GpuSource {
    Nvml,
    Pdh,
}

pub struct GpuSampler {
    backend: Backend,
}

enum Backend {
    #[cfg(all(windows, feature = "nvidia"))]
    Nvml(nvml::NvmlBackend),
    #[cfg(windows)]
    Pdh(pdh::PdhBackend),
    Unavailable,
}

impl GpuSampler {
    pub fn new() -> Self {
        Self { backend: detect() }
    }

    pub fn sample(&mut self) -> Vec<GpuMetrics> {
        match &mut self.backend {
            #[cfg(all(windows, feature = "nvidia"))]
            Backend::Nvml(b) => b.sample(),
            #[cfg(windows)]
            Backend::Pdh(b) => b.sample(),
            Backend::Unavailable => Vec::new(),
        }
    }
}

impl Default for GpuSampler {
    fn default() -> Self {
        Self::new()
    }
}

fn detect() -> Backend {
    #[cfg(all(windows, feature = "nvidia"))]
    if let Some(backend) = nvml::NvmlBackend::new() {
        tracing::info!("GPU backend: NVML");
        return Backend::Nvml(backend);
    }

    #[cfg(windows)]
    if let Some(backend) = pdh::PdhBackend::new() {
        tracing::info!("GPU backend: PDH performance counters");
        return Backend::Pdh(backend);
    }

    tracing::warn!("no GPU backend available; GPU panels will report unknown");
    Backend::Unavailable
}
