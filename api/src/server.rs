use crate::{task, TaskRequest};
use axum::{routing::post, Json, Router};
use std::net::SocketAddr;
use tower_http::trace::TraceLayer;

async fn task_submit(Json(request): Json<TaskRequest>) {
    // Eventually this should add to a queue, rather than run immediately. There is no way to check
    // the result of the task. In the future the client will use a separate endpoint to query the
    // status.
    std::thread::spawn(move || {
        let result = task::execute(&request);
        if let Err(err) = result {
            println!("Task failed to start: {}", err);
        }
    });
}

#[tokio::main(flavor = "current_thread")]
pub async fn start(address: &SocketAddr) {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .init();

    let app = Router::new()
        .route("/task", post(task_submit))
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(address).await.unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
