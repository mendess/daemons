//! A module to emulate cronjobs with daemon manager.
//!
//! A [`Cron`] is a special kind of task that never terminates and run at a configured interval.

use super::{ControlFlow, Daemon};
use chrono::{NaiveDateTime, NaiveTime, Utc};
use futures::Future;
use std::marker::PhantomData;

/// A Cron job.
///
/// This type implements the [`Daemon`] trait such that once submitted to a
/// [`DaemonManager`](super::DaemonManager) it shall behave like a
/// [cronjob](https://linux.die.net/man/5/crontab).
///
/// The generics of this type are its configuration.
/// - `F`: The routine to run when the cronjob runs.
/// - `Fut`: The future the routine returns.
/// - `T`: The context object that will be passed to the routine.
/// - `H`: The hour to run the routine at, must be between 0 and 23 (inclusive).
/// - `M`: The minute within the hour the routine will run at.
/// - `S`: The second within the minute the routine will run at.
///
/// A typical example of how to use this type is as follows:
/// ```
/// use daemons::{DaemonManager, ControlFlow, cron::Cron};
/// use std::sync::Arc;
///
/// // create a more specific type which already contains the time configuration.
/// type SaySomethingJob<F, Fut, T> = Cron<F, Fut, T, 9, 0, 0>;
///
/// let mut daemon = DaemonManager::spawn(Arc::new(()));
///
/// // pass a new instance of the configured type to the daemon manager, notice that all the
/// // generics are inferred
/// daemon.add_daemon(SaySomethingJob::new(
///     "hello",
///     |_| async {
///         println!("hello!");
///         ControlFlow::CONTINUE
///     }
/// ));
/// ```
///
/// When used in conjunction with the [monomophise](super::monomophise::monomophise) macro, you can
/// also omit the data parameter, like you can with the DaemonManager;
///
/// As such the above example can be edited like so:
/// ```
/// use daemons::monomorphise;
///
/// monomorphise!(&'static str);
///
/// type SaySomethingJob<F, Fut> = Cron<F, Fut, 9, 0, 0>;
///
/// // -- omitted creation and adding of a cron to the daemon manager --
/// ```
pub struct Cron<F, Fut, T, const H: u32, const M: u32, const S: u32>
where
    F: FnMut(&T) -> Fut,
    Fut: Future<Output = ControlFlow>,
{
    name: String,
    run: F,
    _marker: PhantomData<T>,
}

impl<F, Fut, T, const H: u32, const M: u32, const S: u32> Cron<F, Fut, T, H, M, S>
where
    F: FnMut(&T) -> Fut,
    Fut: Future<Output = ControlFlow>,
{
    /// Create a new cronjob with a custom routine.
    pub fn new(name: impl Into<String>, run: F) -> Self {
        Self {
            name: name.into(),
            run,
            _marker: PhantomData,
        }
    }
}

#[async_trait::async_trait]
impl<F, Fut, T, const H: u32, const M: u32, const S: u32> Daemon<false> for Cron<F, Fut, T, H, M, S>
where
    F: FnMut(&T) -> Fut + Send + Sync,
    Fut: Future<Output = ControlFlow> + Send ,
    T: Send + Sync,
{
    type Data = T;

    async fn interval(&self) -> std::time::Duration {
        let now = Utc::now().naive_utc();
        let h_m_s = NaiveTime::from_hms_opt(H, M, S).expect("valid h m s");
        let mut target = NaiveDateTime::new(now.date(), h_m_s);
        if now > target {
            target = NaiveDateTime::new(
                target
                    .date()
                    .succ_opt()
                    .expect("not to reach the end of time"),
                target.time(),
            );
        }
        let dur = (target - now).to_std().unwrap_or_default();
        log::trace!(
            "cron task {} will happen in {}",
            self.name,
            humantime::format_duration(dur)
        );
        dur
    }

    async fn name(&self) -> String {
        self.name.clone()
    }

    async fn run(&mut self, data: &Self::Data) -> ControlFlow {
        (self.run)(data).await
    }
}
