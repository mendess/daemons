use chrono::{NaiveDateTime, Timelike, Utc};
use daemons::*;
use std::{sync::Arc, time::Duration};

struct Foo;

#[daemons::async_trait]
impl Daemon for Foo {
    type Data = ();
    async fn run(&mut self, _: &Self::Data) -> ControlFlow {
        log::info!("{:?} ola", Utc::now());
        ControlFlow::CONTINUE
    }

    async fn interval(&self) -> Duration {
        let now = Utc::now().naive_utc();
        let now_zero_nano = now.with_nanosecond(0).unwrap();
        let next_second = now_zero_nano.time() + chrono::Duration::seconds(1);
        let mut target = NaiveDateTime::new(now.date(), next_second);
        if now > target {
            target = NaiveDateTime::new(target.date().succ(), next_second);
        }
        let dur = (target - now).to_std().unwrap_or_default();
        dur
    }

    async fn name(&self) -> String {
        "ola".into()
    }
}

#[tokio::main]
async fn main() {
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Trace)
        .init()
        .unwrap();
    let mut mng = DaemonManager::from(Arc::new(()));
    mng.add_daemon(Foo).await; // thread::spawn

    std::io::stdin().read_line(&mut String::new()).unwrap();
}
