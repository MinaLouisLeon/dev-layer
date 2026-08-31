//! NVIDIA backend. Optional (`nvidia` feature) and always fallible: a machine
//! with no NVIDIA driver simply falls through to the PDH backend.

use nvml_wrapper::enum_wrappers::device::TemperatureSensor;
use nvml_wrapper::Nvml;

use super::{GpuMetrics, GpuSource};

pub struct NvmlBackend {
    nvml: Nvml,
    device_count: u32,
}

impl NvmlBackend {
    pub fn new() -> Option<Self> {
        let nvml = Nvml::init()
            .inspect_err(|e| tracing::debug!(error = %e, "NVML unavailable"))
            .ok()?;
        let device_count = nvml.device_count().ok()?;
        (device_count > 0).then_some(Self { nvml, device_count })
    }

    pub fn sample(&mut self) -> Vec<GpuMetrics> {
        (0..self.device_count)
            .filter_map(|index| {
                let device = self.nvml.device_by_index(index).ok()?;
                let memory = device.memory_info().ok();

                Some(GpuMetrics {
                    name: device.name().unwrap_or_else(|_| format!("GPU {index}")),
                    utilization: device.utilization_rates().ok().map(|u| u.gpu as f32),
                    memory_used: memory.as_ref().map(|m| m.used),
                    memory_total: memory.as_ref().map(|m| m.total),
                    temperature_c: device.temperature(TemperatureSensor::Gpu).ok(),
                    source: GpuSource::Nvml,
                })
            })
            .collect()
    }
}
