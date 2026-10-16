//! Operation manager: every long task (extraction, campaign, product / brand analysis, watch, login
//! windows) is a job with an id, a live state and its own Stop. Several jobs may run side by side;
//! the shared browser profile is handed from one to the next by `scanner::launch` (see `Lease`).
//!
//! Tauri-free on purpose (tested in tests-harness): the app plugs a notifier in at start-up that
//! sends every change to the window as the `job-update` event.
use crate::scanner::ScanEvent;
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex, OnceLock};
use tokio_util::sync::CancellationToken;

/// Jobs that may not run twice at once (the same bank / account / watch list would be touched twice).
const SINGLE: &[&str] = &["scan", "campaign", "watch", "catalog"];
/// More than this many jobs at once only slows every one of them down.
pub const MAX_JOBS: usize = 4;
const RECENT: usize = 12;

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JobInfo {
    pub id: u64,
    pub kind: String,
    pub title: String,
    /// running · waiting_for_user · cancelling · completed · completed_with_warnings · partial ·
    /// cancelled · failed
    pub state: String,
    /// What it is doing now («جستجو در ترب (۱ از ۳)»).
    pub step: String,
    pub checked: i64,
    pub target: i64,
    pub found: i64,
    pub warnings: Vec<String>,
    pub message: String,
    pub started_at: i64,
    pub updated_at: i64,
    pub finished_at: i64,
    /// Started by the scheduler, not by the user.
    pub scheduled: bool,
}

impl JobInfo {
    pub fn active(&self) -> bool {
        matches!(self.state.as_str(), "running" | "waiting_for_user" | "cancelling")
    }
}

type Notify = Box<dyn Fn(&JobInfo) + Send + Sync>;

#[derive(Default)]
struct Inner {
    next: AtomicU64,
    /// (token, info): active jobs first, then the most recent finished ones.
    list: Mutex<Vec<(CancellationToken, JobInfo)>>,
    notify: OnceLock<Notify>,
}

#[derive(Clone, Default)]
pub struct Jobs(Arc<Inner>);

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

pub const BUSY_SAME: &str = "همین عملیات هم‌اکنون در حال اجراست؛ آن را از پنل «عملیات» دنبال یا متوقف کنید.";
pub const BUSY_MANY: &str = "چند عملیات هم‌زمان در حال اجراست؛ یکی را تمام یا متوقف کنید.";

impl Jobs {
    /// Called once at start-up with the function that tells the window about a change.
    pub fn on_change(&self, f: impl Fn(&JobInfo) + Send + Sync + 'static) {
        let _ = self.0.notify.set(Box::new(f));
    }

    fn changed(&self, info: &JobInfo) {
        if let Some(f) = self.0.notify.get() {
            f(info);
        }
    }

    pub fn start(&self, kind: &str, title: &str, scheduled: bool) -> Result<Job, String> {
        let mut list = self.0.list.lock().unwrap_or_else(|e| e.into_inner());
        let running: Vec<&JobInfo> = list.iter().map(|(_, i)| i).filter(|i| i.active()).collect();
        if SINGLE.contains(&kind) && running.iter().any(|i| i.kind == kind) {
            return Err(BUSY_SAME.into());
        }
        if running.len() >= MAX_JOBS {
            return Err(BUSY_MANY.into());
        }
        let id = self.0.next.fetch_add(1, Relaxed) + 1;
        let token = CancellationToken::new();
        let t = now();
        let info = JobInfo {
            id,
            kind: kind.into(),
            title: title.into(),
            state: "running".into(),
            step: "در حال شروع…".into(),
            started_at: t,
            updated_at: t,
            scheduled,
            ..Default::default()
        };
        list.insert(0, (token.clone(), info.clone()));
        drop(list);
        self.changed(&info);
        Ok(Job { jobs: self.clone(), id, token, done: false })
    }

    fn update(&self, id: u64, f: impl FnOnce(&mut JobInfo)) {
        let snapshot = {
            let mut list = self.0.list.lock().unwrap_or_else(|e| e.into_inner());
            let Some((_, info)) = list.iter_mut().find(|(_, i)| i.id == id) else { return };
            f(info);
            info.updated_at = now();
            info.clone()
        };
        self.changed(&snapshot);
    }

    /// Stop one job (`Some(id)`) or every running job (`None`). True when something was stopped.
    pub fn stop(&self, id: Option<u64>) -> bool {
        let stopped: Vec<u64> = {
            let list = self.0.list.lock().unwrap_or_else(|e| e.into_inner());
            list.iter()
                .filter(|(_, i)| i.active() && id.is_none_or(|x| x == i.id))
                .map(|(t, i)| {
                    t.cancel();
                    i.id
                })
                .collect()
        };
        for j in &stopped {
            self.update(*j, |i| {
                if i.active() {
                    i.state = "cancelling".into();
                    i.step = "در حال توقف و بستن مرورگر…".into();
                }
            });
        }
        !stopped.is_empty()
    }

    pub fn list(&self) -> Vec<JobInfo> {
        self.0.list.lock().unwrap_or_else(|e| e.into_inner()).iter().map(|(_, i)| i.clone()).collect()
    }

    pub fn running(&self) -> Vec<JobInfo> {
        self.list().into_iter().filter(JobInfo::active).collect()
    }

    /// Forget finished jobs (the panel's «پاک کردن»).
    pub fn clear_finished(&self) {
        self.0.list.lock().unwrap_or_else(|e| e.into_inner()).retain(|(_, i)| i.active());
    }
}

/// A running job. Dropping it without `finish` (a panic, an early `?`) still ends it as failed or
/// cancelled, so no job is ever left «running» in the panel.
pub struct Job {
    jobs: Jobs,
    pub id: u64,
    pub token: CancellationToken,
    done: bool,
}

impl Jobs {
    /// Applies one progress event of a job to its live state.
    pub fn record(&self, id: u64, e: &ScanEvent) {
        self.update(id, |i| match e {
            ScanEvent::Status { message } => {
                i.step = message.clone();
                if i.state == "waiting_for_user" {
                    i.state = "running".into();
                }
            }
            ScanEvent::Progress { checked, found, target, .. } => {
                i.checked = *checked;
                i.found = *found;
                i.target = *target;
            }
            ScanEvent::Sending { sent, failed, skipped, total } => {
                i.checked = sent + failed + skipped;
                i.found = *sent;
                i.target = *total;
            }
            ScanEvent::Captcha { message } => {
                i.state = "waiting_for_user".into();
                i.step = message.clone();
            }
            ScanEvent::Log { level, message } if matches!(*level, "warning" | "error") => {
                if i.warnings.len() < 30 {
                    i.warnings.push(message.clone());
                }
            }
            _ => {}
        });
    }
}

impl Job {
    /// An event sink that keeps the job's state and forwards every event to `inner` (the page).
    pub fn emitter(&self, inner: impl Fn(ScanEvent) + Send + Sync + 'static) -> impl Fn(ScanEvent) + Send + Sync + 'static {
        let jobs = self.jobs.clone();
        let id = self.id;
        move |e: ScanEvent| {
            jobs.record(id, &e);
            inner(e);
        }
    }

    /// Ends the job. `error` = failed; else cancelled / partial (stopped) or completed (with warnings).
    pub fn finish(mut self, error: Option<&str>, message: &str) {
        self.close(error, message);
    }

    fn close(&mut self, error: Option<&str>, message: &str) {
        if self.done {
            return;
        }
        self.done = true;
        let cancelled = self.token.is_cancelled();
        self.jobs.update(self.id, |i| {
            i.state = match (error, cancelled) {
                (Some(_), false) => "failed",
                (_, true) if i.found > 0 || i.checked > 0 => "partial",
                (_, true) => "cancelled",
                (None, false) if !i.warnings.is_empty() => "completed_with_warnings",
                (None, false) => "completed",
            }
            .into();
            i.message = error.map_or_else(|| message.to_string(), str::to_string);
            i.step.clear();
            i.finished_at = now();
        });
        let mut list = self.jobs.0.list.lock().unwrap_or_else(|e| e.into_inner());
        // active jobs first, then the latest finished ones
        list.sort_by_key(|(_, i)| (!i.active(), std::cmp::Reverse(i.finished_at.max(i.started_at)), std::cmp::Reverse(i.id)));
        let mut finished = 0;
        list.retain(|(_, i)| i.active() || {
            finished += 1;
            finished <= RECENT
        });
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        if !self.done {
            self.close(Some("عملیات به‌طور ناگهانی پایان یافت؛ داده‌های ذخیره‌شده حفظ شده‌اند."), "");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle() {
        let jobs = Jobs::default();
        let seen = Arc::new(Mutex::new(vec![]));
        let s2 = seen.clone();
        jobs.on_change(move |i| s2.lock().unwrap().push(i.state.clone()));
        let a = jobs.start("market", "عسل", false).unwrap();
        let b = jobs.start("market", "زعفران", false).unwrap();
        assert_eq!(jobs.running().len(), 2, "two analyses side by side");
        let scan = jobs.start("scan", "x", false).unwrap();
        assert_eq!(jobs.start("scan", "y", false).err().as_deref(), Some(BUSY_SAME));
        let _c = jobs.start("brand", "z", false).unwrap();
        assert_eq!(jobs.start("brand", "w", false).err().as_deref(), Some(BUSY_MANY));
        let emit = a.emitter(|_| {});
        emit(ScanEvent::Progress { keyword: "عسل".into(), collected: 5, checked: 1, found: 3, target: 3 });
        emit(ScanEvent::Captcha { message: "تأیید".into() });
        assert_eq!(jobs.list().iter().find(|i| i.id == a.id).unwrap().state, "waiting_for_user");
        emit(ScanEvent::Status { message: "ادامه".into() });
        assert!(jobs.stop(Some(a.id)));
        assert_eq!(jobs.list().iter().find(|i| i.id == a.id).unwrap().state, "cancelling");
        assert!(a.token.is_cancelled() && !b.token.is_cancelled(), "Stop is per job");
        let aid = a.id;
        a.finish(None, "متوقف شد");
        assert_eq!(jobs.list().iter().find(|i| i.id == aid).unwrap().state, "partial", "stopped with data = partial");
        b.finish(Some("خطا"), "");
        drop(scan); // dropped without finish (panic / early return)
        let l = jobs.list();
        assert!(l.iter().any(|i| i.state == "failed" && i.message == "خطا"));
        assert_eq!(l.iter().filter(|i| i.state == "failed").count(), 2, "{l:?}");
        assert_eq!(jobs.running().len(), 1);
        assert!(l[0].active(), "active jobs listed first");
        jobs.clear_finished();
        assert_eq!(jobs.list().len(), 1);
        assert!(seen.lock().unwrap().contains(&"cancelling".to_string()));
    }
}
