use crate::server::{ApiServer, Configuration};
use crate::task::{TaskInfo, TaskRequest, TaskStatus};
use crate::{worker, Client};
use assertor::*;
use reqwest::Url;
use std::collections::HashMap;
use std::future::IntoFuture;
use std::time::{Duration, Instant};
use tokio::runtime;
use tokio::sync::oneshot;

struct BackgroundRuntime(#[allow(unused)] oneshot::Sender<()>);

/// Starts a server in a background thread and returns a `Client` that is configured to interact
/// with it.
///
/// The server needs a Tokio runtime to run. When the returned `BackgroundRuntime` object is
/// dropped, this informs the runtime that it needs to shutdown.
fn start_server() -> anyhow::Result<(Client, BackgroundRuntime)> {
    let rt = runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    let config = Configuration {
        polling_interval: Duration::from_secs(0),
    };
    let app = rt.block_on(ApiServer::new(config))?;

    // Bind on a random port and then find out what port was used so we can create the client.
    let listener = rt.block_on(app.listen("127.0.0.1:0"))?;
    let addr = listener.local_addr()?;

    // This channel is used to shutdown the background thread. We never actually write anything to
    // it, but dropping the sender will unblock the receiver.
    let (tx, rx) = oneshot::channel();

    std::thread::spawn(move || {
        rt.block_on(async {
            tokio::spawn(listener.into_future());
            // This will block until the sender is dropped, at which point we return, stop the
            // runtime and close the server.
            let _ = rx.await;
        });
    });

    let url = Url::parse(&format!("http://{addr}"))?;
    let client = Client::new(url);
    Ok((client, BackgroundRuntime(tx)))
}

/// Block until the given function returns true.
///
/// This is useful in tests that need to wait for something to happen asynchronously.
/// It will poll the function repeatedly until the condition is true or some timeout has elapsed.
/// If the function returns an error that error is propagated without retries.
fn wait_for<F>(f: F) -> anyhow::Result<()>
where
    F: Fn() -> anyhow::Result<bool>,
{
    // Nothing we test for should take a long time. 1 second is plenty of time.
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if f()? {
            return Ok(());
        }
    }
    anyhow::bail!("timeout")
}

#[test_log::test]
fn can_submit_task() -> anyhow::Result<()> {
    let (client, _rt) = start_server()?;
    let _worker = worker::run_background(client.url().clone());

    let id = client.task_submit(&TaskRequest {
        environment: HashMap::new(),
        cmdline: vec!["true".to_owned()],
    })?;

    wait_for(|| Ok(client.task_get(id)?.status.is_finished()))?;

    assert_eq!(
        client.task_get(id)?,
        TaskInfo {
            id,
            status: TaskStatus::Complete
        }
    );

    Ok(())
}

#[test_log::test]
fn failed_tasks_have_appropriate_status() -> anyhow::Result<()> {
    let (client, _rt) = start_server()?;
    let _worker = worker::run_background(client.url().clone());

    // These two commands fail in different ways: the first one has a non-zero exit code whereas
    // the second one does not even start. We don't yet have any way of distinguishing between the
    // two cases.
    let id1 = client.task_submit(&TaskRequest {
        environment: HashMap::new(),
        cmdline: vec!["false".to_owned()],
    })?;
    let id2 = client.task_submit(&TaskRequest {
        environment: HashMap::new(),
        cmdline: vec!["not-a-real-command".to_owned()],
    })?;

    wait_for(|| Ok(client.task_get(id1)?.status.is_finished()))?;
    wait_for(|| Ok(client.task_get(id2)?.status.is_finished()))?;

    assert_eq!(client.task_get(id1)?.status, TaskStatus::Failed);
    assert_eq!(client.task_get(id2)?.status, TaskStatus::Failed);

    Ok(())
}

#[test_log::test]
fn can_list_tasks() -> anyhow::Result<()> {
    let (client, _rt) = start_server()?;
    let _worker = worker::run_background(client.url().clone());

    let id1 = client.task_submit(&TaskRequest {
        environment: HashMap::new(),
        cmdline: vec!["true".to_owned()],
    })?;
    let id2 = client.task_submit(&TaskRequest {
        environment: HashMap::new(),
        cmdline: vec!["false".to_owned()],
    })?;

    wait_for(|| Ok(client.task_get(id1)?.status.is_finished()))?;
    wait_for(|| Ok(client.task_get(id2)?.status.is_finished()))?;

    let tasks = client.task_list()?;
    assert_that!(tasks).contains_exactly(vec![
        TaskInfo {
            id: id1,
            status: TaskStatus::Complete,
        },
        TaskInfo {
            id: id2,
            status: TaskStatus::Failed,
        },
    ]);

    Ok(())
}
