//! Vendor-neutral GPU load via PDH performance counters — the same data
//! Task Manager's GPU graphs come from.
//!
//! Two counters, both wildcard instances:
//!   * `\GPU Engine(*)\Utilization Percentage`
//!   * `\GPU Process Memory(*)\Dedicated Usage`
//!
//! Instance names look like
//! `pid_1234_luid_0x00000000_0x0000ABCD_phys_0_eng_0_engtype_3D`.
//!
//! **Aggregation matters**: Task Manager reports the *busiest engine type*
//! (3D, Copy, VideoDecode…), not the sum across all of them and not their
//! average. Summing would happily report 300 %. So: sum instances within each
//! engine type, then take the max across types.

use std::collections::HashMap;

use windows::core::w;
use windows::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
    PdhOpenQueryW, PDH_CSTATUS_VALID_DATA, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE,
    PDH_HCOUNTER, PDH_HQUERY, PDH_MORE_DATA,
};

use super::{GpuMetrics, GpuSource};

const ERROR_SUCCESS: u32 = 0;

pub struct PdhBackend {
    query: PDH_HQUERY,
    utilization: PDH_HCOUNTER,
    dedicated_memory: PDH_HCOUNTER,
    /// PDH needs two collections before a counter yields a value; the first
    /// sample is reported as unknown rather than as zero.
    primed: bool,
}

impl PdhBackend {
    pub fn new() -> Option<Self> {
        unsafe {
            let mut query = PDH_HQUERY::default();
            if PdhOpenQueryW(None, 0, &mut query) != ERROR_SUCCESS {
                tracing::debug!("PdhOpenQueryW failed; no PDH GPU counters");
                return None;
            }

            let mut utilization = PDH_HCOUNTER::default();
            let mut dedicated_memory = PDH_HCOUNTER::default();

            // English counter names so this works on localized Windows.
            let util_ok = PdhAddEnglishCounterW(
                query,
                w!("\\GPU Engine(*)\\Utilization Percentage"),
                0,
                &mut utilization,
            ) == ERROR_SUCCESS;
            let mem_ok = PdhAddEnglishCounterW(
                query,
                w!("\\GPU Process Memory(*)\\Dedicated Usage"),
                0,
                &mut dedicated_memory,
            ) == ERROR_SUCCESS;

            if !util_ok {
                tracing::debug!("GPU Engine counter unavailable");
                let _ = PdhCloseQuery(query);
                return None;
            }
            if !mem_ok {
                tracing::debug!("GPU Process Memory counter unavailable; VRAM will be unknown");
            }

            // Prime the query; the first collection establishes a baseline.
            let _ = PdhCollectQueryData(query);

            Some(Self {
                query,
                utilization,
                dedicated_memory,
                primed: false,
            })
        }
    }

    pub fn sample(&mut self) -> Vec<GpuMetrics> {
        unsafe {
            if PdhCollectQueryData(self.query) != ERROR_SUCCESS {
                return vec![unknown()];
            }
            if !self.primed {
                self.primed = true;
                return vec![unknown()];
            }

            let utilization = busiest_engine(&read_counter(self.utilization));
            let memory_used: u64 = read_counter(self.dedicated_memory)
                .iter()
                .map(|(_, value)| *value as u64)
                .sum();

            vec![GpuMetrics {
                name: "GPU".into(),
                utilization,
                memory_used: (memory_used > 0).then_some(memory_used),
                // PDH exposes usage, never capacity.
                memory_total: None,
                temperature_c: None,
                source: GpuSource::Pdh,
            }]
        }
    }
}

impl Drop for PdhBackend {
    fn drop(&mut self) {
        unsafe {
            let _ = PdhCloseQuery(self.query);
        }
    }
}

fn unknown() -> GpuMetrics {
    GpuMetrics {
        name: "GPU".into(),
        utilization: None,
        memory_used: None,
        memory_total: None,
        temperature_c: None,
        source: GpuSource::Pdh,
    }
}

/// Sums instances per engine type, returns the busiest type's total.
fn busiest_engine(samples: &[(String, f64)]) -> Option<f32> {
    if samples.is_empty() {
        return None;
    }

    let mut by_type: HashMap<&str, f64> = HashMap::new();
    for (instance, value) in samples {
        let engine_type = instance
            .rsplit_once("engtype_")
            .map(|(_, t)| t)
            .unwrap_or("unknown");
        *by_type.entry(engine_type).or_default() += value;
    }

    by_type
        .into_values()
        .fold(None::<f64>, |max, v| Some(max.map_or(v, |m| m.max(v))))
        .map(|v| v.clamp(0.0, 100.0) as f32)
}

/// Reads every instance of a wildcard counter as `(instance name, value)`.
fn read_counter(counter: PDH_HCOUNTER) -> Vec<(String, f64)> {
    let mut buffer_size = 0u32;
    let mut item_count = 0u32;

    // First call sizes the buffer and is expected to "fail" with PDH_MORE_DATA.
    let status = unsafe {
        PdhGetFormattedCounterArrayW(
            counter,
            PDH_FMT_DOUBLE,
            &mut buffer_size,
            &mut item_count,
            None,
        )
    };
    if status != PDH_MORE_DATA || item_count == 0 {
        return Vec::new();
    }

    // Allocate as items rather than bytes so the buffer is correctly aligned.
    let item_size = std::mem::size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>();
    let capacity = (buffer_size as usize).div_ceil(item_size);
    let mut items: Vec<PDH_FMT_COUNTERVALUE_ITEM_W> = vec![Default::default(); capacity];

    let status = unsafe {
        PdhGetFormattedCounterArrayW(
            counter,
            PDH_FMT_DOUBLE,
            &mut buffer_size,
            &mut item_count,
            Some(items.as_mut_ptr()),
        )
    };
    if status != ERROR_SUCCESS {
        return Vec::new();
    }

    items
        .iter()
        .take(item_count as usize)
        .filter(|item| item.FmtValue.CStatus == PDH_CSTATUS_VALID_DATA)
        .filter_map(|item| {
            let name = unsafe { item.szName.to_string() }.ok()?;
            Some((name, unsafe { item.FmtValue.Anonymous.doubleValue }))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::busiest_engine;

    fn instance(engine: &str) -> String {
        format!("pid_1000_luid_0x0_0x1234_phys_0_eng_0_engtype_{engine}")
    }

    #[test]
    fn reports_busiest_engine_type_not_the_sum() {
        let samples = vec![
            (instance("3D"), 40.0),
            (instance("3D"), 25.0),
            (instance("Copy"), 10.0),
            (instance("VideoDecode"), 5.0),
        ];
        // 3D sums to 65; summing every type would report 80.
        assert_eq!(busiest_engine(&samples), Some(65.0));
    }

    #[test]
    fn clamps_and_handles_empty() {
        assert_eq!(busiest_engine(&[]), None);
        assert_eq!(busiest_engine(&[(instance("3D"), 140.0)]), Some(100.0));
    }
}
