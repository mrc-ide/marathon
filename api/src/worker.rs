use crate::task::{TaskId, TaskRequest, TaskResult, TaskStatus};
use crate::Client;
use reqwest::Url;
use std::sync::mpsc;
use std::time::Duration;

#[tracing::instrument(skip_all, fields(task = %id))]
fn process_task(client: &Client, id: TaskId, request: &TaskRequest) -> crate::Result<()> {
    client.queue_update(id, TaskStatus::Running)?;

    let status = match crate::execute(request) {
        Ok(TaskResult { status, .. }) => {
            if status.success() {
                TaskStatus::Complete
            } else {
                TaskStatus::Failed
            }
        }
        Err(_) => TaskStatus::Failed,
    };

    client.queue_update(id, status)?;

    Ok(())
}

#[tracing::instrument(name = "worker", skip_all)]
fn run_inner(url: Url, shutdown: mpsc::Receiver<()>) -> anyhow::Result<()> {
    let client = Client::new(url);
    loop {
        tracing::info!("Polling for new tasks");
        match client.queue_pull()? {
            Ok(data) => process_task(&client, data.task, &data.request)?,
            Err(interval) => {
                let interval = interval.unwrap_or(Duration::from_secs(5));
                match shutdown.recv_timeout(interval) {
                    Ok(_) => unreachable!(),
                    Err(mpsc::RecvTimeoutError::Timeout) => (),
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        }
    }
    Ok(())
}

pub fn run(url: Url) -> crate::Result<()> {
    let (_tx, rx) = mpsc::channel();
    run_inner(url, rx)
}

pub struct BackgroundWorker(#[allow(unused)] mpsc::Sender<()>);
pub fn run_background(url: Url) -> BackgroundWorker {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        if let Err(error) = run_inner(url, rx) {
            tracing::error!(%error, "background worker thread failed");
        }
    });
    BackgroundWorker(tx)
}
