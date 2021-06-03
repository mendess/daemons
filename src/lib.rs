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
    pub async fn run_one(&mut self, i: usize) -> Result<(), usize> {
        match OptFut::from(self.channels.get(&i).map(|(_, c)| c.send(Msg::Run))).await {
            Some(Ok(_)) => Ok(()),
            _ => Err(i),
        }
    }

    pub async fn run_all(&mut self) -> Result<(), usize> {
        for (i, (_, c)) in self.channels.iter() {
            c.send(Msg::Run).await.map_err(|_| *i)?
        }
        Ok(())
    }

    pub async fn cancel(&mut self, i: usize) -> Result<(), usize> {
        if let None = OptFut::from(self.channels.get(&i).map(|(_, c)| c.send(Msg::Cancel))).await {
            Err(i)
        } else {
            Ok(())
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
                let _ = timeout(next_run - now, rx.recv()).await;

                if daemon.lock().await.run(&data).await.is_break() {
                    break;
                }
            }
        });
        self.next_id += Wrapping(1);
        id.0
    }

    pub fn daemon_names(&self) -> impl Iterator<Item = (usize, &str)> {
        self.channels.iter().map(|(i, (n, _))| (*i, n.as_str()))
    }

    pub fn start(data: Arc<D>) -> Self {
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
