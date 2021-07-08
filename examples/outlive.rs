use daemons::*;
use std::{sync::Arc, time::Duration};

struct Foo(i32);

#[daemons::async_trait]
impl Daemon for Foo {
    type Data = ();
    async fn name(&self) -> String {
        "ola".into()
    }

    async fn interval(&self) -> Duration {
        Duration::from_secs(1)
    }

    async fn run(&mut self, _: &Self::Data) -> ControlFlow {
        println!("ola");
        self.0 += 1;
        if self.0 == 3 {
            ControlFlow::BREAK
        } else {
            ControlFlow::CONTINUE
        }
    }
}

#[tokio::main]
async fn main() {
    simple_logger::SimpleLogger::new().with_level(log::LevelFilter::Trace).init().unwrap();
    {
        let mut mng = DaemonManager::from(Arc::new(()));
        mng.add_daemon(Foo(0)).await; // thread::spawn
    }

    std::io::stdin().read_line(&mut String::new()).unwrap();
}
