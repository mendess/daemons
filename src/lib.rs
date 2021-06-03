#![deny(unused_crate_dependencies)]
#![deny(rust_2018_idioms)]
#![deny(unsafe_code)]
#![deny(unused_must_use)]

use futures::future::OptionFuture as OptFut;
use std::{
    collections::HashMap,
    num::Wrapping,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    sync::{
        mpsc::{self, Sender},
        Mutex,
    },
    time::timeout,
};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ControlFlow {
    Continue,
    Break,
}

impl ControlFlow {
    pub fn is_break(self) -> bool {
        self == ControlFlow::Break
    }

    pub fn is_continue(self) -> bool {
        !self.is_break()
    }
}

/// A Daemon
#[async_trait::async_trait]
pub trait Daemon {
    type Data;

    /// What the daemon does when it runs
    async fn run(&mut self, data: &Self::Data) -> ControlFlow;
    /// How frequent the daemon must run
    async fn interval(&self) -> Duration;
    /// The name of the daemon
    async fn name(&self) -> String;
}

enum Msg {
    Run,
    Cancel,
}

pub struct DaemonThread<Data> {
    next_id: Wrapping<usize>,
    channels: HashMap<usize, (String, Sender<Msg>)>,
    data: Arc<Data>,
}

impl<D: Send + Sync + 'static> DaemonThread<D> {
    async fn send_msg(&mut self, i: usize, msg: Msg) -> Result<(), usize> {
        match OptFut::from(self.channels.get(&i).map(|(_, c)| c.send(msg))).await {
            Some(Ok(_)) => Ok(()),
            e => {
                if e.is_some() {
                    self.channels.remove(&i);
                }
                Err(i)
            }
        }
    }

    pub async fn run_one(&mut self, i: usize) -> Result<(), usize> {
        self.send_msg(i, Msg::Run).await
    }

    pub async fn cancel(&mut self, i: usize) -> Result<(), usize> {
        self.send_msg(i, Msg::Cancel).await
    }

    pub async fn run_all(&mut self) -> Result<(), Vec<usize>> {
        let mut failed = Vec::new();
        for (i, (_, c)) in self.channels.iter() {
            if let Err(_) = c.send(Msg::Run).await {
                failed.push(*i)
            }
        }
        if failed.is_empty() {
            Ok(())
        } else {
            failed.iter().for_each(|i| {
                self.channels.remove(&i);
            });
            Err(failed)
        }
    }

    pub async fn add_daemon<T>(&mut self, mut daemon: T) -> usize
    where
        T: Daemon<Data = D> + Send + Sync + 'static,
    {
        let id = self.next_id;
        let (sx, mut rx) = mpsc::channel(10);
        self.channels.insert(id.0, (daemon.name().await, sx));
        let data = self.data.clone();
        tokio::spawn(async move {
            loop {
                let now = Instant::now();
                let next_run = now + daemon.interval().await;
                //TODO: or patterns
                if let Ok(Some(Msg::Cancel)) | Ok(None) = timeout(next_run - now, rx.recv()).await
                {
                    break;
                }

                if daemon.run(&data).await.is_break() {
                    break;
                }
            }
        });
        self.next_id += Wrapping(1);
        id.0
    }

    pub async fn add_shared<T>(&mut self, daemon: Arc<Mutex<T>>) -> usize
    where
        T: Daemon<Data = D> + Send + Sync + 'static,
    {
        let id = self.next_id;
        let (sx, mut rx) = mpsc::channel(10);
        self.channels
            .insert(id.0, (daemon.lock().await.name().await, sx));
        let data = self.data.clone();
        tokio::spawn(async move {
            loop {
                let now = Instant::now();
                let next_run = now + daemon.lock().await.interval().await;
                if let Ok(Some(Msg::Cancel)) | Ok(None) = timeout(next_run - now, rx.recv()).await
                {
                    break;
                }

                if daemon.lock().await.run(&data).await.is_break() {
                    break;
                }
            }
        });
        self.next_id += Wrapping(1);
        id.0
    }

    pub fn daemon_names(&self) -> impl Iterator<Item = (usize, &str)> {
        self.channels
            .iter()
            .filter(|(_, (_, s))| !s.is_closed())
            .map(|(i, (n, _))| (*i, n.as_str()))
    }

    pub fn spawn(data: Arc<D>) -> Self {
        data.into()
    }
}

impl<D: Send + Sync + 'static> From<Arc<D>> for DaemonThread<D> {
    fn from(data: Arc<D>) -> Self {
        Self {
            next_id: Wrapping(0),
            channels: HashMap::new(),
            data,
        }
    }
}
