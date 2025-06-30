use crate::{server::create_app, Client, TaskRequest};
use reqwest::Url;
use std::collections::HashMap;
use std::future::IntoFuture;
use tokio::net::TcpListener;
use tokio::runtime;
use tokio::sync::oneshot;

struct BackgroundRuntime(#[allow(unused)] oneshot::Sender<()>);

// Starts a server in a background thread and returns a Client setup to communicate with it.
//
// The server needs a tokio runtime to run. When the returned `BackgroundRuntime` object is
// dropped, this informs the runtime that it needs to shutdown.
fn start() -> anyhow::Result<(Client, BackgroundRuntime)> {
    let rt = runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    let listener = rt.block_on(TcpListener::bind("0.0.0.0:0"))?;
    let addr = listener.local_addr()?;

    let (tx, rx) = oneshot::channel();
    std::thread::spawn(move || {
        rt.block_on(async {
            let app = create_app();
            tokio::spawn(axum::serve(listener, app).into_future());
            // This will block until the sender is dropped, at which point we return, stop the
            // runtime and close the server.
            rx.await
        })
        .unwrap();
    });

    let url = Url::parse(&format!("http://{addr}"))?;
    let client = Client::new(url);
    Ok((client, BackgroundRuntime(tx)))
}

#[test]
fn can_submit_task() -> anyhow::Result<()> {
    let (client, _rt) = start()?;
    let id = client.submit(&TaskRequest {
        environment: HashMap::new(),
        cmdline: vec!["ls".to_owned()],
    })?;

    // Not the most useful test but this is pretty much the only thing that is exposed for now.
    assert_eq!(id.get_version(), Some(uuid::Version::Random));

    Ok(())
}
