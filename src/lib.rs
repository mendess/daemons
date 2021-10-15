#![deny(unused_crate_dependencies)]
#![deny(rust_2018_idioms)]
#![deny(unsafe_code)]
#![deny(unused_must_use)]

mod control_flow;
mod monomorphise;

pub use async_trait::async_trait;
pub use control_flow::ControlFlow;
use futures::future::OptionFuture as OptFut;
use std::{
    any::{type_name, TypeId},
    collections::HashMap,
    num::Wrapping,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{
    sync::{
        mpsc::{self, Sender},
        Mutex, RwLock,
    },
    time::timeout,
};

/// A Daemon, daemons run at specified intervals (or when asked to) until they return
/// [ControlFlow::Break].
///
/// COMPENSATE_INTERVAL: Whether the value returned from the interval should have run time subtracted
#[async_trait]
pub trait Daemon<const COMPENSATE_INTERVAL: bool> {
    type Data;

    /// What the daemon does when it runs
    async fn run(&mut self, data: &Self::Data) -> ControlFlow;
    /// How frequent the daemon must run
    async fn interval(&self) -> Duration;
    /// The name of the daemon
    async fn name(&self) -> String;
}

#[async_trait]
impl<T, const C: bool> Daemon<C> for Arc<Mutex<T>>
where
    T: Daemon<C> + 'static + Send + Sync,
    T::Data: Send + Sync,
{
    type Data = T::Data;

    async fn run(&mut self, data: &Self::Data) -> ControlFlow {
        self.lock().await.run(data).await
    }
    async fn interval(&self) -> Duration {
        self.lock().await.interval().await
    }
    async fn name(&self) -> String {
        self.lock().await.name().await
    }
}

#[async_trait]
impl<T, const C: bool> Daemon<C> for Arc<RwLock<T>>
where
    T: Daemon<C> + 'static + Send + Sync,
    T::Data: Send + Sync,
{
    type Data = T::Data;

    async fn run(&mut self, data: &Self::Data) -> ControlFlow {
        self.write().await.run(data).await
    }
    async fn interval(&self) -> Duration {
        self.read().await.interval().await
    }
    async fn name(&self) -> String {
        self.read().await.name().await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Msg {
    Run,
    Cancel,
}

/// A daemon thread handle used to spawn more daemons or to force daemons to run at arbitrary times
#[derive(Debug)]
pub struct DaemonManager<Data> {
    next_id: Wrapping<usize>,
    channels: HashMap<usize, DaemonHandle>,
    data: Arc<Data>,
    needs_gc: Arc<AtomicBool>,
}

/// A daemon handle, this will provide the name and the original type id of the associated daemon
#[derive(Debug)]
pub struct DaemonHandle {
    name: String,
    ch: Sender<Msg>,
    ty: TypeId,
}

impl DaemonHandle {
    fn new<T: 'static>(name: String, ch: Sender<Msg>) -> Self {
        Self {
            name,
            ch,
            ty: TypeId::of::<T>(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn ty(&self) -> TypeId {
        self.ty
    }
}

impl<D: Send + Sync + 'static> DaemonManager<D> {
    async fn send_msg(&mut self, i: usize, msg: Msg) -> Result<(), usize> {
        self.gc();
        let send_fut = self.channels.get(&i).map(|dhandle| {
            log::trace!("Sending message {:?} to daemon '{}'", msg, dhandle.name);
            dhandle.ch.send(msg)
        });
        match OptFut::from(send_fut).await {
            Some(Ok(_)) => Ok(()),
            e => {
                if e.is_some() {
                    self.channels.remove(&i);
                }
                Err(i)
            }
        }
    }

    #[inline(always)]
    fn gc(&mut self) {
        if self.needs_gc.load(Ordering::Relaxed) {
            self.channels.retain(|_, v| !v.ch.is_closed());
            self.needs_gc.store(false, Ordering::Relaxed);
        }
    }

    /// Run daemon of id `i` now.
    ///
    /// # Errors
    /// If the passed daemon is not running, the passed id is returned
    pub async fn run_one(&mut self, i: usize) -> Result<(), usize> {
        self.send_msg(i, Msg::Run).await
    }

    /// Cancel a daemon, removing it from the pool of daemons.
    ///
    /// Canceling a daemon more than once is a no op.
    ///
    /// # Errors
    /// If the passed daemon is not running, the passed id is returned
    pub async fn cancel(&mut self, i: usize) -> Result<(), usize> {
        self.send_msg(i, Msg::Cancel).await?;
        self.channels.remove(&i);
        Ok(())
    }

    /// Run all daemons now
    pub async fn run_all(&mut self) {
        log::trace!("Running all {} daemons", self.channels.len());
        let mut failed = Vec::new();
        for (i, dhandle) in self.channels.iter() {
            if dhandle.ch.send(Msg::Run).await.is_err() {
                failed.push(*i)
            }
        }
        failed.iter().for_each(|i| {
            self.channels.remove(i);
        });
        self.needs_gc.store(false, Ordering::Relaxed);
    }

    /// Start a new daemon
    pub async fn add_daemon<T, const COMPENSATE: bool>(&mut self, mut daemon: T) -> usize
    where
        T: Daemon<COMPENSATE, Data = D> + Send + Sync + 'static,
    {
        let name = daemon.name().await;
        log::trace!("Adding daemon {}({:?})", type_name::<T>(), name);
        let id = self.next_id;
        let (sx, mut rx) = mpsc::channel(10);
        self.channels
            .insert(id.0, DaemonHandle::new::<T>(name.clone(), sx));
        let data = self.data.clone();
        tokio::spawn({
            let needs_gc = self.needs_gc.clone();
            async move {
                let mut last_run = Instant::now();
                loop {
                    let mut interval = daemon.interval().await;
                    let now = Instant::now();
                    if COMPENSATE {
                        interval = interval.saturating_sub(now - last_run);
                    }
                    let mut loop_count = 0;
                    while let Some(time_left) = interval.checked_sub(now.elapsed()) {
                        if loop_count > 0 {
                            log::trace!(
                                "Daemon {}({:?}) finished sleeping with {:?} time left. loop count: {}",
                                type_name::<T>(),
                                name,
                                time_left,
                                loop_count
                            );
                        }
                        match timeout(time_left, rx.recv()).await {
                            Ok(Some(Msg::Cancel)) => break,
                            Ok(Some(Msg::Run)) => (),
                            Ok(None) => {
                                tokio::time::sleep(interval).await;
                                if COMPENSATE {
                                    last_run = Instant::now();
                                }
                            }
                            Err(_) if COMPENSATE => last_run = Instant::now(),
                            Err(_) => {}
                        }
                        loop_count += 1;
                    }

                    if daemon.run(&data).await.is_break() {
                        needs_gc.store(true, Ordering::Relaxed);
                        break;
                    }
                }
            }
        });
        self.next_id += Wrapping(1);
        self.gc();
        id.0
    }

    /// List all names of all running daemons
    pub fn daemon_names(&self) -> impl Iterator<Item = (usize, &DaemonHandle)> {
        self.channels
            .iter()
            .filter(move |(_, dhandle)| {
                if dhandle.ch.is_closed() {
                    self.needs_gc.store(true, Ordering::Relaxed);
                    false
                } else {
                    true
                }
            })
            .map(|(i, dhandle)| (*i, dhandle))
    }

    /// Create a daemon thread, this doesn't start task, it simply converts the data object
    /// into a [Self]
    pub fn spawn(data: Arc<D>) -> Self {
        data.into()
    }
}

impl<D: Send + Sync + 'static> From<Arc<D>> for DaemonManager<D> {
    fn from(data: Arc<D>) -> Self {
        Self {
            next_id: Wrapping(0),
            channels: HashMap::new(),
            data,
            needs_gc: Default::default(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[test]
    fn shared() {
        struct Foo;
        #[async_trait::async_trait]
        impl Daemon<false> for Foo {
            type Data = ();
            async fn name(&self) -> String {
                "ola".into()
            }

            async fn interval(&self) -> Duration {
                Duration::default()
            }

            async fn run(&mut self, _: &Self::Data) -> ControlFlow {
                ControlFlow::BREAK
            }
        }

        let _ = DaemonManager::from(Arc::new(())).add_daemon(Arc::new(Mutex::new(Foo)));
    }
}
