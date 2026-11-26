//! Memory budget and swap-pressure watch against oversized or hostile files (T-03).
//!
//! Files opened from disk can be arbitrarily large or crafted (e.g. deeply
//! nested JSON bombs). Running out of memory aborts a Rust process without a
//! catchable error, so every file import obtains a [`MemoryBudget`] bounded by
//! the machine's available RAM (minus a safety margin), refuses files that
//! obviously cannot fit before reading them, and checks live process allocation
//! during parsing so a runaway file aborts cleanly while preserving the open
//! project.

use std::cell::Cell;
use std::fmt;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use serde::de::{self, DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::Value;

/// Conservative expansion multiplier for `.trazo`/`.json` project files in RAM.
pub const PROJECT_FACTOR: u64 = 8;
/// Conservative expansion multiplier for ASCII DXF files during import.
pub const DXF_FACTOR: u64 = 10;
/// Conservative expansion multiplier for raster/SVG/PDF underlay decoding.
pub const UNDERLAY_FACTOR: u64 = 16;
/// Conservative expansion multiplier for future IFC/STEP imports.
#[allow(dead_code)]
pub const IFC_FACTOR: u64 = 14;

/// Fallback file-size caps applied when no process meter is installed (e.g. unit tests).
pub const MAX_PROJECT_FALLBACK_BYTES: u64 = 256 << 20;
pub const MAX_DXF_FALLBACK_BYTES: u64 = 256 << 20;
pub const MAX_UNDERLAY_FALLBACK_BYTES: u64 = 128 << 20;

/// Minimum RAM headroom reserved for the OS and other applications (256 MiB, or
/// 10% of currently free RAM, whichever is larger).
const MARGIN_BYTES: u64 = 256 << 20;

/// Swap growth threshold (512 MiB) before reporting active swapping.
const SWAPPING_THRESHOLD_BYTES: u64 = 512 << 20;

/// Process and host memory probes installed once at startup.
#[derive(Clone, Copy)]
pub struct Meter {
    pub used: fn() -> usize,
    pub free: fn() -> Option<u64>,
}

static METER: OnceLock<Meter> = OnceLock::new();

/// Installs the global memory meter for the lifetime of the process.
pub fn install(meter: Meter) {
    let _ = METER.set(meter);
}

/// Current live heap allocation reported by the installed meter, in bytes.
#[allow(dead_code)]
pub fn used() -> Option<usize> {
    METER.get().map(|m| (m.used)())
}

/// Currently available machine RAM reported by the installed meter, in bytes.
#[allow(dead_code)]
pub fn free() -> Option<u64> {
    METER.get().and_then(|m| (m.free)())
}

/// Snapshot of host swap and RAM availability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pressure {
    pub swap_used: u64,
    pub available: u64,
    pub total: u64,
}

static PRESSURE: OnceLock<fn() -> Option<Pressure>> = OnceLock::new();

/// Installs the host swap/memory pressure probe.
pub fn install_pressure(read: fn() -> Option<Pressure>) {
    let _ = PRESSURE.set(read);
}

/// Reads current host memory pressure, if a probe is installed.
#[allow(dead_code)]
pub fn pressure() -> Option<Pressure> {
    PRESSURE.get().and_then(|read| read())
}

/// Installs the default `sysinfo`-backed meter and pressure probes using the
/// provided allocator byte counter (e.g. `cap::Cap::allocated`).
pub fn install_system_meter(used_bytes: fn() -> usize) {
    install(Meter {
        used: used_bytes,
        free: system_free_memory,
    });
    install_pressure(system_pressure);
}

/// Memory at or below which a computer is treated as modest (#164): the
/// 4 GB laptops many families have, with what the system reserves.
pub const MODEST_COMPUTER_BYTES: u64 = 4_600 * 1024 * 1024;

/// Whether this computer has little memory, so Trazo should start in light
/// mode. `false` when the memory cannot be read.
pub fn is_modest_computer() -> bool {
    let Ok(mut sys) = sysinfo::System::new() else {
        return false;
    };
    sys.refresh_memory();
    let total = sys.total_memory();
    total > 0 && total <= MODEST_COMPUTER_BYTES
}

fn system_free_memory() -> Option<u64> {
    let mut sys = sysinfo::System::new().ok()?;
    sys.refresh_memory();
    let available = sys.available_memory();
    let free = sys
        .cgroup_limits()
        .map_or(available, |limits| limits.free_memory.min(available));
    (free > 0).then_some(free)
}

fn system_pressure() -> Option<Pressure> {
    let mut sys = sysinfo::System::new().ok()?;
    sys.refresh_memory();
    Some(Pressure {
        swap_used: sys.used_swap(),
        available: sys.available_memory(),
        total: sys.total_memory(),
    })
}

/// Edge-triggered swap-pressure detector with hysteresis so the UI warns once
/// when the system starts thrashing swap and clears once memory recovers.
#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct SwapWatch {
    baseline: Option<u64>,
    swapping: bool,
}

#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq)]
pub enum SwapEvent {
    /// Swap usage grew by at least `SWAPPING_THRESHOLD_BYTES` while free RAM dropped below 10%.
    Began(u64),
    /// Swap pressure subsided or free RAM recovered above 20%.
    Ended,
}

#[allow(dead_code)]
impl SwapWatch {
    pub fn sample(&mut self, p: Pressure) -> Option<SwapEvent> {
        let base = *self.baseline.get_or_insert(p.swap_used);
        if p.swap_used < base {
            self.baseline = Some(p.swap_used);
        }
        let floor = self.baseline.unwrap_or(base);
        let grown = p.swap_used.saturating_sub(floor);
        let short = p.total > 0 && p.available < p.total / 10;
        match (self.swapping, grown >= SWAPPING_THRESHOLD_BYTES && short) {
            (false, true) => {
                self.swapping = true;
                Some(SwapEvent::Began(grown))
            }
            (true, false) if grown < SWAPPING_THRESHOLD_BYTES / 2 || p.available > p.total / 5 => {
                self.swapping = false;
                Some(SwapEvent::Ended)
            }
            _ => None,
        }
    }

    pub fn is_swapping(&self) -> bool {
        self.swapping
    }
}

/// Memory budget guarding a single file load or import operation.
pub struct MemoryBudget {
    meter: Option<(fn() -> usize, usize)>,
    room: u64,
    stopped: Cell<bool>,
}

impl MemoryBudget {
    /// Unlimited budget (used when no meter is active and fallback size checks passed).
    pub fn none() -> Self {
        Self {
            meter: None,
            room: 0,
            stopped: Cell::new(false),
        }
    }

    /// Creates a budget for reading a file at `path` whose in-memory footprint
    /// is roughly `factor` times its byte size. Falls back to `fallback_max_bytes`
    /// when no live OS meter is installed.
    pub fn for_file(path: &Path, factor: u64, fallback_max_bytes: u64) -> Result<Self, String> {
        let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        match Self::start(size, factor)? {
            Some(budget) => Ok(budget),
            None => {
                if size > fallback_max_bytes {
                    return Err(format!(
                        "O arquivo tem {} e excede o limite de segurança de {} para abertura.",
                        format_bytes(size),
                        format_bytes(fallback_max_bytes)
                    ));
                }
                Ok(Self::none())
            }
        }
    }

    /// Starts a budget using the globally installed [`Meter`], returning `Ok(None)`
    /// when no meter is installed or free RAM cannot be queried.
    pub fn start(size: u64, factor: u64) -> Result<Option<Self>, String> {
        METER
            .get()
            .map_or(Ok(None), |meter| Self::with(meter, size, factor))
    }

    /// Starts a budget against an explicit [`Meter`] (useful in unit tests).
    pub fn with(meter: &Meter, size: u64, factor: u64) -> Result<Option<Self>, String> {
        let Some(free) = (meter.free)() else {
            return Ok(None);
        };
        let headroom = (free / 10).max(MARGIN_BYTES);
        let room = free.saturating_sub(headroom);
        let estimated_need = size.saturating_mul(factor);
        if estimated_need > room {
            return Err(format!(
                "Este arquivo exige cerca de {} de memória, mas há apenas {} livres: feche outros programas e tente novamente.",
                format_bytes(estimated_need),
                format_bytes(free)
            ));
        }
        let stop = (meter.used)().saturating_add(usize::try_from(room).unwrap_or(usize::MAX));
        Ok(Some(Self {
            meter: Some((meter.used, stop)),
            room,
            stopped: Cell::new(false),
        }))
    }

    /// Checks whether the operation is still within its memory ceiling.
    /// Once exceeded, the failure is latched so callers still receive the
    /// budget error even after temporary allocations have been dropped.
    pub fn check(&self) -> Result<(), String> {
        let exceeded = self.meter.is_some_and(|(used_fn, stop)| used_fn() > stop);
        if exceeded {
            self.stopped.set(true);
        }
        if self.stopped.get() {
            Err(format!(
                "A leitura excedeu o limite seguro de {} de memória livre e foi interrompida sem alterar o projeto aberto.",
                format_bytes(self.room)
            ))
        } else {
            Ok(())
        }
    }

    /// Remaining bytes before the budget trips, if metered.
    #[allow(dead_code)]
    pub fn left(&self) -> Option<u64> {
        self.meter
            .map(|(used_fn, stop)| stop.saturating_sub(used_fn()) as u64)
    }
}

/// Human-friendly byte formatter (MB below 1 GB, GB with one decimal above).
pub fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.1} GB", bytes as f64 / 1e9)
    } else {
        format!("{} MB", bytes.div_ceil(1_000_000).max(1))
    }
}

/// Parses JSON text into a [`serde_json::Value`] while checking `budget` after
/// every array element and object field so hostile JSON bombs abort cleanly.
pub fn read_json_metered(text: &str, budget: &MemoryBudget) -> Result<Value, String> {
    budget.check()?;
    let mut de = serde_json::Deserializer::from_str(text);
    MeteredValue(budget)
        .deserialize(&mut de)
        .and_then(|value| de.end().map(|()| value))
        .map_err(|error| budget.check().err().unwrap_or_else(|| error.to_string()))
}

struct MeteredValue<'a>(&'a MemoryBudget);

impl<'de> DeserializeSeed<'de> for MeteredValue<'_> {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for MeteredValue<'_> {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("qualquer valor JSON")
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Value, E> {
        Ok(Value::Bool(v))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Value, E> {
        Ok(Value::from(v))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Value, E> {
        Ok(Value::from(v))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Value, E> {
        Ok(Value::from(v))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Value, E> {
        Ok(Value::String(v.to_owned()))
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<Value, E> {
        Ok(Value::String(v))
    }

    fn visit_none<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut out = Vec::new();
        while let Some(v) = seq.next_element_seed(MeteredValue(self.0))? {
            self.0.check().map_err(<A::Error as de::Error>::custom)?;
            out.push(v);
        }
        Ok(Value::Array(out))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut out = serde_json::Map::new();
        while let Some(k) = map.next_key::<String>()? {
            let v = map.next_value_seed(MeteredValue(self.0))?;
            self.0.check().map_err(<A::Error as de::Error>::custom)?;
            out.insert(k, v);
        }
        Ok(Value::Object(out))
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use super::{MARGIN_BYTES, MemoryBudget, Meter, PROJECT_FACTOR};
    use std::cell::Cell;

    thread_local! {
        static USED: Cell<usize> = const { Cell::new(0) };
        static FREE: Cell<Option<u64>> = const { Cell::new(None) };
    }

    /// Thread-local simulated meter: reports `free` available RAM and adds 1 MiB
    /// of simulated allocation on each `used()` probe.
    pub(crate) fn fake_meter(free: Option<u64>) -> Meter {
        FREE.with(|f| f.set(free));
        USED.with(|u| u.set(0));
        Meter {
            used: || USED.with(|u| u.replace(u.get() + (1 << 20))),
            free: || FREE.with(Cell::get),
        }
    }

    /// Builds a simulated budget with `room_bytes` of headroom above `MARGIN_BYTES`.
    pub(crate) fn fake_budget(room_bytes: u64) -> MemoryBudget {
        MemoryBudget::with(
            &fake_meter(Some(MARGIN_BYTES + room_bytes)),
            0,
            PROJECT_FACTOR,
        )
        .ok()
        .flatten()
        .unwrap_or_else(MemoryBudget::none)
    }
}

#[cfg(test)]
mod tests {
    use super::testing::{fake_budget, fake_meter};
    use super::*;

    #[test]
    fn refuses_oversized_file_before_reading() {
        let meter = fake_meter(Some(4 << 30)); // 4 GiB free -> ~3.8 GiB room
        let err = MemoryBudget::with(&meter, 600 << 20, PROJECT_FACTOR)
            .err()
            .unwrap();
        assert!(err.contains("exige cerca de"), "{err}");
        assert!(
            MemoryBudget::with(&meter, 300 << 20, PROJECT_FACTOR)
                .unwrap()
                .is_some()
        );
        assert!(
            MemoryBudget::with(&fake_meter(None), u64::MAX, PROJECT_FACTOR)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn metered_json_aborts_runaway_allocations_cleanly() {
        let bomb = format!("[{}]", vec!["0"; 512].join(","));
        let budget = fake_budget(8_000_000);
        let err = read_json_metered(&bomb, &budget).unwrap_err();
        assert!(err.contains("excedeu o limite seguro"), "{err}");
        // Even after dropping temporary data, the latched error remains reported.
        fake_meter(Some(0));
        assert!(budget.check().is_err());
    }

    #[test]
    fn metered_json_matches_standard_serde_json() {
        let sample =
            r#"{"n":42,"neg":-7,"f":3.5,"ok":true,"nil":null,"arr":[1,"a",{}],"obj":{"k":"v"}}"#;
        let metered = read_json_metered(sample, &MemoryBudget::none()).unwrap();
        let plain: Value = serde_json::from_str(sample).unwrap();
        assert_eq!(metered, plain);
        assert!(read_json_metered("{invalid", &MemoryBudget::none()).is_err());
    }

    #[test]
    fn swap_watch_detects_onset_and_recovery_with_hysteresis() {
        let gb = 1u64 << 30;
        let snap = |swap: u64, avail: u64| Pressure {
            swap_used: swap,
            available: avail,
            total: 16 * gb,
        };
        let mut watch = SwapWatch::default();
        assert_eq!(watch.sample(snap(2 * gb, 8 * gb)), None);
        assert_eq!(watch.sample(snap(3 * gb, 8 * gb)), None);
        assert_eq!(watch.sample(snap(3 * gb, gb)), Some(SwapEvent::Began(gb)));
        assert_eq!(watch.sample(snap(3 * gb + 50, gb / 2)), None);
        assert!(watch.is_swapping());
        assert_eq!(watch.sample(snap(3 * gb, 6 * gb)), Some(SwapEvent::Ended));
        assert!(!watch.is_swapping());
    }
}
