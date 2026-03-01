use anyhow::{anyhow, Result};
use std::{future::Future, time::Duration};
use tokio::{
    sync::{oneshot, Mutex},
    time,
};

const TIMEOUT: Duration = Duration::from_secs(3);

pub type SResult<T> = Result<T, String>;

pub struct TaskResult<T> {
    lock: Mutex<()>,
    tx: Mutex<Option<oneshot::Sender<T>>>,
}

impl<T> TaskResult<T> {
    pub fn new() -> Self {
        TaskResult {
            lock: Mutex::default(),
            tx: Mutex::default(),
        }
    }
    pub async fn acquire<F>(&self, f: impl FnOnce() -> F) -> Result<T>
    where
        F: Future<Output = Result<()>>,
    {
        let _guard = self.lock.lock().await;
        let (tx, rx) = oneshot::channel();
        *self.tx.lock().await = Some(tx);
        f().await?;
        Ok(time::timeout(TIMEOUT, rx).await??)
    }
    pub async fn put(&self, value: T) -> Result<()> {
        self.tx
            .lock()
            .await
            .take()
            .ok_or_else(|| anyhow!("no active task"))?
            .send(value)
            .map_err(|_| anyhow!("failed to send value"))
    }
}
