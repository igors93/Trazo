//! Background analysis worker (#108): computes structural reports,
//! reinforcement proposals and electrical load schedules off the UI thread
//! while feeding the shared content-fingerprinted caches (`domain::cache`),
//! coalescing rapid edits ("latest wins") so large projects stay responsive.

use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::thread::{self, JoinHandle};

use super::{
    cache::{
        electrical_fingerprint, store_load_schedule, store_reinforcement_report,
        store_structure_report, structure_fingerprint,
    },
    electrical::LoadSchedule,
    model::Project,
    reinforcement::{ReinforcementReport, build_reinforcement_report},
    structure::StructureReport,
};

type WakeFn = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalysisKind {
    Structure,
    Electrical,
}

struct PendingJobs {
    structure: Option<(u64, Project)>,
    electrical: Option<(u64, Project)>,
    shutdown: bool,
}

/// Shared worker state driving one persistent background analysis thread.
struct WorkerState {
    jobs: Mutex<PendingJobs>,
    notify: Condvar,
    running_structure: AtomicBool,
    running_electrical: AtomicBool,
    active_structure_key: AtomicU64,
    active_electrical_key: AtomicU64,
    completed_jobs: AtomicU64,
    last_worker_thread_id: Mutex<Option<thread::ThreadId>>,
    wake: Mutex<Option<WakeFn>>,
}

/// Non-blocking background analysis coordinator owned by the application.
///
/// Repeated edits while the worker is busy overwrite the queued snapshot
/// ("latest wins"), avoiding wasted work and preventing UI frame hitches.
pub struct BackgroundAnalyzer {
    state: Arc<WorkerState>,
    handle: Mutex<Option<JoinHandle<()>>>,
    last_structure: Mutex<Option<Arc<StructureReport>>>,
    last_reinforcement: Mutex<Option<Arc<ReinforcementReport>>>,
    last_electrical: Mutex<Option<Arc<LoadSchedule>>>,
}

impl Default for BackgroundAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl BackgroundAnalyzer {
    pub fn new() -> Self {
        let state = Arc::new(WorkerState {
            jobs: Mutex::new(PendingJobs {
                structure: None,
                electrical: None,
                shutdown: false,
            }),
            notify: Condvar::new(),
            running_structure: AtomicBool::new(false),
            running_electrical: AtomicBool::new(false),
            active_structure_key: AtomicU64::new(0),
            active_electrical_key: AtomicU64::new(0),
            completed_jobs: AtomicU64::new(0),
            last_worker_thread_id: Mutex::new(None),
            wake: Mutex::new(None),
        });
        Self {
            state,
            handle: Mutex::new(None),
            last_structure: Mutex::new(None),
            last_reinforcement: Mutex::new(None),
            last_electrical: Mutex::new(None),
        }
    }

    fn ensure_worker(&self) {
        let mut slot = self.handle.lock().unwrap_or_else(|e| e.into_inner());
        if slot.is_none() {
            let worker_state = Arc::clone(&self.state);
            *slot = thread::Builder::new()
                .name("trazo-analysis-worker".to_owned())
                .spawn(move || worker_loop(worker_state))
                .ok();
        }
    }

    /// Registers a callback invoked whenever a background job completes (e.g.
    /// `egui::Context::request_repaint`).
    pub fn set_wake(&self, wake: impl Fn() + Send + Sync + 'static) {
        let mut guard = self.state.wake.lock().unwrap_or_else(|e| e.into_inner());
        *guard = Some(Arc::new(wake));
    }

    /// True while either structural or electrical analysis is queued or running.
    pub fn is_busy(&self) -> bool {
        if self.state.running_structure.load(Ordering::Acquire)
            || self.state.running_electrical.load(Ordering::Acquire)
        {
            return true;
        }
        let guard = self.state.jobs.lock().unwrap_or_else(|e| e.into_inner());
        guard.structure.is_some() || guard.electrical.is_some()
    }

    /// Human-readable status indicator for the status bar while computing.
    pub fn busy_label(&self) -> Option<&'static str> {
        let s = self.state.running_structure.load(Ordering::Acquire);
        let e = self.state.running_electrical.load(Ordering::Acquire);
        if s && e {
            Some("Calculando estrutura e elétrica...")
        } else if s {
            Some("Calculando estrutura...")
        } else if e {
            Some("Calculando elétrica...")
        } else {
            let guard = self.state.jobs.lock().unwrap_or_else(|e| e.into_inner());
            if guard.structure.is_some() {
                Some("Calculando estrutura...")
            } else if guard.electrical.is_some() {
                Some("Calculando elétrica...")
            } else {
                None
            }
        }
    }

    /// Schedules a structural analysis (report + reinforcement) in the
    /// background if the current fingerprint is not already cached or in flight.
    pub fn schedule_structure(&self, project: &Project) {
        if project.peek_cached_structure_report().is_some()
            && project.peek_cached_reinforcement_report().is_some()
        {
            return;
        }
        let key = structure_fingerprint(project);
        if self.state.running_structure.load(Ordering::Acquire)
            && self.state.active_structure_key.load(Ordering::Acquire) == key
        {
            return;
        }
        let mut guard = self.state.jobs.lock().unwrap_or_else(|e| e.into_inner());
        if guard.structure.as_ref().map(|(k, _)| *k) == Some(key) {
            return;
        }
        guard.structure = Some((key, project.clone()));
        drop(guard);
        self.ensure_worker();
        self.state.notify.notify_one();
    }

    /// Schedules an electrical load-schedule calculation in the background if
    /// the current fingerprint is not already cached or in flight.
    pub fn schedule_electrical(&self, project: &Project) {
        if project.peek_cached_load_schedule().is_some() {
            return;
        }
        let key = electrical_fingerprint(project);
        if self.state.running_electrical.load(Ordering::Acquire)
            && self.state.active_electrical_key.load(Ordering::Acquire) == key
        {
            return;
        }
        let mut guard = self.state.jobs.lock().unwrap_or_else(|e| e.into_inner());
        if guard.electrical.as_ref().map(|(k, _)| *k) == Some(key) {
            return;
        }
        guard.electrical = Some((key, project.clone()));
        drop(guard);
        self.ensure_worker();
        self.state.notify.notify_one();
    }

    /// Non-blocking structural report fetch: returns the cached report if warm;
    /// otherwise schedules background computation and returns the most recent
    /// completed report (marked `stale = true`) or computes once on first use.
    pub fn structure_report_or_schedule(&self, project: &Project) -> (Arc<StructureReport>, bool) {
        if let Some(hit) = project.peek_cached_structure_report() {
            *self
                .last_structure
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Some(Arc::clone(&hit));
            return (hit, false);
        }
        self.schedule_structure(project);
        if let Some(previous) = self
            .last_structure
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
        {
            return (previous, true);
        }
        // Cold first call: return an empty default immediately while the worker
        // finishes so the UI frame never stalls on large plans.
        (Arc::new(StructureReport::default()), true)
    }

    /// Non-blocking reinforcement report fetch paired with [`Self::structure_report_or_schedule`].
    pub fn reinforcement_report_or_schedule(
        &self,
        project: &Project,
    ) -> (Arc<ReinforcementReport>, bool) {
        if let Some(hit) = project.peek_cached_reinforcement_report() {
            *self
                .last_reinforcement
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Some(Arc::clone(&hit));
            return (hit, false);
        }
        self.schedule_structure(project);
        if let Some(previous) = self
            .last_reinforcement
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
        {
            return (previous, true);
        }
        (Arc::new(ReinforcementReport::default()), true)
    }

    /// Non-blocking electrical load schedule fetch.
    pub fn load_schedule_or_schedule(&self, project: &Project) -> (Arc<LoadSchedule>, bool) {
        if let Some(hit) = project.peek_cached_load_schedule() {
            *self
                .last_electrical
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Some(Arc::clone(&hit));
            return (hit, false);
        }
        self.schedule_electrical(project);
        if let Some(previous) = self
            .last_electrical
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
        {
            return (previous, true);
        }
        (Arc::new(LoadSchedule::default()), true)
    }

    /// Total completed background analysis passes (diagnostic/test helper).
    #[cfg(test)]
    pub fn completed_jobs(&self) -> u64 {
        self.state.completed_jobs.load(Ordering::Acquire)
    }
}

impl Drop for BackgroundAnalyzer {
    fn drop(&mut self) {
        {
            let mut guard = self.state.jobs.lock().unwrap_or_else(|e| e.into_inner());
            guard.shutdown = true;
            self.state.notify.notify_all();
        }
        if let Some(handle) = self.handle.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = handle.join();
        }
    }
}

fn worker_loop(state: Arc<WorkerState>) {
    {
        let mut tid = state
            .last_worker_thread_id
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *tid = Some(thread::current().id());
    }
    loop {
        let next = {
            let mut guard = state.jobs.lock().unwrap_or_else(|e| e.into_inner());
            while !guard.shutdown && guard.structure.is_none() && guard.electrical.is_none() {
                guard = state.notify.wait(guard).unwrap_or_else(|e| e.into_inner());
            }
            if guard.shutdown {
                return;
            }
            if let Some(job) = guard.structure.take() {
                (AnalysisKind::Structure, job)
            } else if let Some(job) = guard.electrical.take() {
                (AnalysisKind::Electrical, job)
            } else {
                continue;
            }
        };

        match next {
            (AnalysisKind::Structure, (key, project)) => {
                state.active_structure_key.store(key, Ordering::Release);
                state.running_structure.store(true, Ordering::Release);
                let report = Arc::new(project.structure_report());
                let reinforcement = Arc::new(build_reinforcement_report(&project, &report));
                store_structure_report(key, report);
                store_reinforcement_report(key, reinforcement);
                state.running_structure.store(false, Ordering::Release);
                state.completed_jobs.fetch_add(1, Ordering::AcqRel);
            }
            (AnalysisKind::Electrical, (key, project)) => {
                state.active_electrical_key.store(key, Ordering::Release);
                state.running_electrical.store(true, Ordering::Release);
                let schedule = Arc::new(project.load_schedule());
                store_load_schedule(key, schedule);
                state.running_electrical.store(false, Ordering::Release);
                state.completed_jobs.fetch_add(1, Ordering::AcqRel);
            }
        }

        if let Some(wake) = state.wake.lock().unwrap_or_else(|e| e.into_inner()).clone() {
            wake();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;
    use std::time::{Duration, Instant};

    use super::*;
    use crate::domain::fixtures::FixtureKind;
    use crate::domain::geometry::Point;

    #[test]
    fn background_analyzer_computes_off_main_thread_populates_cache_and_wakes_ui() {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(20.0, 20.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.5, 3.5), "Sala")
            .unwrap();
        project.generate_structure();
        project.generate_electrical_points();
        let (pw, pd) = FixtureKind::Panel.default_size();
        project
            .add_fixture(FixtureKind::Panel, Point::new(0.4, 0.4), pw, pd, 0)
            .unwrap();
        project.assign_circuits();
        project.generate_conduits().unwrap();
        // Mutate a unique live load so this test never collides with another test's warm slot.
        project.structure.settings.live_kn_m2 = 2.37;
        project.electrical.voltage_v = 127.0;

        let analyzer = BackgroundAnalyzer::new();
        let wakes = Arc::new(AtomicUsize::new(0));
        let wakes_clone = Arc::clone(&wakes);
        analyzer.set_wake(move || {
            wakes_clone.fetch_add(1, Ordering::SeqCst);
        });

        let (_initial, stale) = analyzer.structure_report_or_schedule(&project);
        assert!(
            stale,
            "cold cache should schedule asynchronously without blocking"
        );
        analyzer.schedule_electrical(&project);

        let deadline = Instant::now() + Duration::from_secs(10);
        while (project.peek_cached_structure_report().is_none()
            || project.peek_cached_reinforcement_report().is_none()
            || project.peek_cached_load_schedule().is_none())
            && Instant::now() < deadline
        {
            thread::sleep(Duration::from_millis(10));
        }

        let cached = project
            .peek_cached_structure_report()
            .expect("background worker should populate structure cache");
        assert!(!cached.beams.is_empty());
        assert!(project.peek_cached_reinforcement_report().is_some());
        assert!(project.peek_cached_load_schedule().is_some());

        // Subsequent non-blocking call hits the warm cache instantaneously.
        let (hit, hit_stale) = analyzer.structure_report_or_schedule(&project);
        assert!(!hit_stale);
        assert!(Arc::ptr_eq(&cached, &hit));
        assert!(wakes.load(Ordering::SeqCst) >= 1);
        assert!(analyzer.completed_jobs() >= 2);

        let worker_tid = *analyzer.state.last_worker_thread_id.lock().unwrap();
        assert_ne!(worker_tid, Some(thread::current().id()));
    }
}
