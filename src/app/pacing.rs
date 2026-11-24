//! Smooth frame-pacing policy for desktop viewports (T-07).
//!
//! Tracks per-frame render cost with an exponential moving average (EMA) and
//! computes the repaint delay needed to respect either the monitor's refresh
//! rate, a user-chosen FPS cap, or a CPU-friendly share on software rasterizers
//! (such as Windows WARP or Linux llvmpipe).

use std::time::{Duration, Instant};

const DEFAULT_HZ: f64 = 60.0;
const MIN_HZ: f64 = 10.0;
const MAX_HZ: f64 = 240.0;
/// Share of CPU time allowed when running on a software rasterizer.
const SOFTWARE_CPU_BUDGET: f64 = 0.5;
/// Gaps longer than this represent idle pauses, not frame render cost.
const IDLE_PAUSE: Duration = Duration::from_millis(1500);
const EMA_ALPHA: f64 = 0.3;
const MAX_WAIT: Duration = Duration::from_millis(1000);

/// User-selectable frame-rate ceiling.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FrameRateLimit {
    #[default]
    Auto,
    Hz(u32),
}

impl FrameRateLimit {
    pub fn interval(self, monitor_hz: f64) -> Duration {
        let hz = match self {
            FrameRateLimit::Auto => monitor_hz.clamp(MIN_HZ, MAX_HZ),
            FrameRateLimit::Hz(cap) => f64::from(cap).clamp(MIN_HZ, MAX_HZ),
        };
        Duration::from_secs_f64(1.0 / hz)
    }
}

/// Keep the driver's vsync by default; `TRAZO_VSYNC=0` opts out for diagnostics.
pub fn driver_vsync_requested() -> bool {
    !std::env::var("TRAZO_VSYNC").is_ok_and(|v| v == "0" || v.eq_ignore_ascii_case("false"))
}

/// Detects common CPU/software OpenGL/Vulkan/Direct3D adapter names.
pub fn is_software_renderer(adapter_name: &str) -> bool {
    let lower = adapter_name.to_ascii_lowercase();
    [
        "basic render driver",
        "warp",
        "llvmpipe",
        "lavapipe",
        "swiftshader",
        "softpipe",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

/// Stateful frame pacer keeping continuous animations/drags smooth without
/// pegging the GPU/CPU when no hardware vsync is active.
#[derive(Debug)]
pub struct FramePacer {
    monitor_hz: f64,
    limit: FrameRateLimit,
    software_adapter: bool,
    ema_cost_secs: Option<f64>,
    last_frame: Option<Instant>,
}

impl Default for FramePacer {
    fn default() -> Self {
        // eframe does not expose the GL renderer name without unsafe GL calls.
        // Allow an explicit adapter name for WARP/llvmpipe installations.
        let software_adapter =
            std::env::var("TRAZO_RENDERER").is_ok_and(|name| is_software_renderer(&name));
        let mut pacer = Self::new(DEFAULT_HZ, software_adapter);
        if let Ok(hz) = std::env::var("TRAZO_MAX_FPS")
            && let Ok(hz) = hz.parse::<u32>()
            && hz > 0
        {
            pacer.set_limit(FrameRateLimit::Hz(hz));
        }
        pacer
    }
}

impl FramePacer {
    pub fn new(monitor_hz: f64, software_adapter: bool) -> Self {
        Self {
            monitor_hz: monitor_hz.clamp(MIN_HZ, MAX_HZ),
            limit: FrameRateLimit::Auto,
            software_adapter,
            ema_cost_secs: None,
            last_frame: None,
        }
    }

    pub fn set_limit(&mut self, limit: FrameRateLimit) {
        self.limit = limit;
    }

    /// Records the completion of a frame whose render duration was `cost`, and
    /// returns how long to wait before scheduling the next continuous repaint.
    pub fn record_frame(&mut self, now: Instant, cost: Duration) -> Duration {
        let is_pause = self
            .last_frame
            .is_some_and(|prev| now.saturating_duration_since(prev) > IDLE_PAUSE);
        self.last_frame = Some(now);
        if !is_pause {
            let sample = cost.as_secs_f64().clamp(0.0, 1.0);
            let next = match self.ema_cost_secs {
                Some(prev) => prev * (1.0 - EMA_ALPHA) + sample * EMA_ALPHA,
                None => sample,
            };
            self.ema_cost_secs = Some(next);
        }

        let target = self.limit.interval(self.monitor_hz);
        let software_wait = if self.software_adapter {
            let ema = self.ema_cost_secs.unwrap_or(cost.as_secs_f64());
            Duration::from_secs_f64((ema / SOFTWARE_CPU_BUDGET).min(MAX_WAIT.as_secs_f64()))
        } else {
            Duration::ZERO
        };

        target.max(software_wait).saturating_sub(cost).min(MAX_WAIT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_pacer_respects_monitor_rate_cap_and_software_backoff() {
        assert!(is_software_renderer("Microsoft Basic Render Driver"));
        assert!(is_software_renderer("llvmpipe (LLVM 18.1)"));
        assert!(!is_software_renderer("AMD Radeon RX 6700 XT"));

        let now = Instant::now();
        let mut gpu = FramePacer::new(60.0, false);
        let wait = gpu.record_frame(now, Duration::from_millis(4));
        // 16.67 ms - 4 ms ~= 12.67 ms
        assert!(wait >= Duration::from_millis(11) && wait <= Duration::from_millis(14));

        gpu.set_limit(FrameRateLimit::Hz(30));
        let wait_30 = gpu.record_frame(now + Duration::from_millis(16), Duration::from_millis(3));
        assert!(wait_30 >= Duration::from_millis(29) && wait_30 <= Duration::from_millis(32));

        let mut sw = FramePacer::new(60.0, true);
        let wait_sw = sw.record_frame(now, Duration::from_millis(80));
        // On software adapter: 80 ms / 0.5 - 80 ms = 80 ms backoff.
        assert!(wait_sw >= Duration::from_millis(75));
    }
}
