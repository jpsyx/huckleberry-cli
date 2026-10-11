//! Local-only request and output harness shared by prepared command journeys.
pub(super) use crate::commands::test_support as api;

use crate::{
    prompt::host::{self, Reply, Request},
    session::Context,
};
use anyhow::Result;
use huckleberry_api::{Huckleberry, Session, Zone};

pub fn client(stub: &api::Stub) -> Huckleberry {
    Huckleberry::with_parts(
        reqwest::Client::new(),
        stub.documents_url.clone(),
        None,
        Zone::new("UTC").unwrap(),
        Some(Session {
            id_token: "test".into(),
            refresh_token: "test".into(),
            user_uid: "test".into(),
            expires_at: i64::MAX,
        }),
    )
}

pub struct Fixture(std::path::PathBuf);
impl Fixture {
    pub fn create_fixture(name: &str) -> Self {
        let directory =
            std::env::temp_dir().join(format!("h-prepared-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        Self(directory)
    }
    pub fn context(&self) -> Context {
        Context {
            config: crate::config::Config {
                timezone: "UTC".into(),
                ..Default::default()
            },
            config_path: self.0.join("config.toml"),
            credentials_path: self.0.join("credentials.toml"),
            theme: crate::theme::Theme::dark(false),
            verbose: false,
            child_override: Some("test-child".into()),
            offline: None,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn hosted<Value: Send + 'static>(
    operation: impl FnOnce() -> Result<Value> + Send + 'static,
) -> (Value, Vec<String>) {
    let _serial = host::one_at_a_time();
    let channel = host::install();
    let worker = std::thread::spawn(operation);
    let mut shown = Vec::new();
    while !worker.is_finished() {
        let Ok((request, reply)) = channel
            .requests
            .recv_timeout(std::time::Duration::from_millis(20))
        else {
            continue;
        };
        match request {
            Request::Show(lines) => {
                shown.extend(lines);
                let _ = reply.send(Reply::Shown);
            }
            Request::Step(_) => {
                let _ = reply.send(Reply::Stepped { interrupted: false });
            }
            Request::Frame(lines) => panic!("execution must not collect answers: {lines:?}"),
        }
    }
    host::remove();
    (worker.join().unwrap().unwrap(), shown)
}
