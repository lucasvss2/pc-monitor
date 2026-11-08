//! GPU values share the System I/O seam; native identities never enter snapshots.
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", content = "value", rename_all = "camelCase")]
pub enum GpuReading {
    Ready(f64),
    Measuring,
    Unavailable,
    Failed,
}

impl GpuReading {
    pub fn temperature(value: f64) -> Self {
        if value.is_finite() && (-30.0..=150.0).contains(&value) {
            Self::Ready((value * 10.0).round() / 10.0)
        } else {
            Self::Failed
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gpu {
    /// Display ordinal and vendor only; no PCI address, LUID, PID or device UUID.
    pub label: String,
    pub usage: GpuReading,
    pub temperature: GpuReading,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Graphics {
    pub devices: Vec<Gpu>,
    pub failed: bool,
}

#[derive(Debug, Clone)]
pub struct EngineSample {
    pub adapter: u64,
    pub physical: u32,
    pub engine: u32,
    pub value: f64,
}

impl EngineSample {
    /// Windows GPU Engine instance syntax. Process names/IDs are not retained.
    pub fn parse(instance: &str, value: f64) -> Option<Self> {
        if !value.is_finite() || !(0.0..=100.5).contains(&value) {
            return None;
        }
        let (_, identity) = instance.split_once("_luid_")?;
        let mut parts = identity.split('_');
        let high = u32::from_str_radix(parts.next()?.strip_prefix("0x")?, 16).ok()?;
        let low = u32::from_str_radix(parts.next()?.strip_prefix("0x")?, 16).ok()?;
        if parts.next()? != "phys" {
            return None;
        }
        let physical = parts.next()?.parse().ok()?;
        if parts.next()? != "eng" {
            return None;
        }
        let engine = parts.next()?.parse().ok()?;
        Some(Self {
            adapter: (u64::from(high) << 32) | u64::from(low),
            physical,
            engine,
            value: value.min(100.0),
        })
    }
}

/// Sum processes sharing an engine, then take the busiest engine of this GPU.
pub fn utilization(adapter: u64, samples: &[EngineSample]) -> GpuReading {
    let mut engines = BTreeMap::new();
    for sample in samples.iter().filter(|sample| sample.adapter == adapter) {
        *engines.entry((sample.physical, sample.engine)).or_insert(0.0) += sample.value;
    }
    engines
        .into_values()
        .reduce(f64::max)
        .map(|value| GpuReading::Ready((value.min(100.0) * 10.0).round() / 10.0))
        .unwrap_or(GpuReading::Unavailable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregates_processes_per_engine_without_mixing_adapters_or_nodes() {
        let input: serde_json::Value = serde_json::from_str(include_str!(
            "../../fixtures/system/gpu-engines.json"
        ))
        .unwrap();
        let samples: Vec<_> = input["samples"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|row| EngineSample::parse(row["instance"].as_str()?, row["value"].as_f64()?))
            .collect();
        assert_eq!(utilization(1, &samples), GpuReading::Ready(70.2));
        assert_eq!(utilization(2, &samples), GpuReading::Ready(22.0));
        assert_eq!(utilization(3, &samples), GpuReading::Unavailable);
        let published = serde_json::to_string(&Gpu {
            label: "GPU 0 · NVIDIA".into(),
            usage: utilization(1, &samples),
            temperature: GpuReading::temperature(43.54),
        })
        .unwrap();
        assert!(!published.contains("pid_") && !published.contains("luid_"));
        assert!(!published.contains("PRIVATE"));
    }

    #[test]
    fn missing_invalid_and_zero_values_are_distinct() {
        let instance = "pid_101_luid_0x00000000_0x00000001_phys_0_eng_0_engtype_3D";
        let zero = EngineSample::parse(instance, 0.0).unwrap();
        assert_eq!(utilization(1, &[zero]), GpuReading::Ready(0.0));
        for value in [f64::NAN, f64::INFINITY, -1.0, 999.0] {
            assert!(EngineSample::parse(instance, value).is_none());
        }
        assert!(EngineSample::parse("PRIVATE_PROCESS", 25.0).is_none());
        assert_eq!(GpuReading::temperature(f64::NAN), GpuReading::Failed);
        assert_eq!(GpuReading::temperature(999.0), GpuReading::Failed);
    }
}
