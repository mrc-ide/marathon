use crate::messages::{ApiResponse, TaskSubmitResponse};
use crate::{task, TaskRequest};
use axum::{routing::post, Json, Router};
use std::net::SocketAddr;
use tower_http::trace::TraceLayer;
use tracing::Span;
use uuid::Uuid;

async fn task_submit(Json(request): Json<TaskRequest>) -> ApiResponse<TaskSubmitResponse> {
    // Eventually this should add to a queue, rather than run immediately. There is no way to check
    // the result of the task based on the ID. In the future the client will use a separate
    // endpoint to query the status.
    let id = Uuid::new_v4();

    let span = Span::current();
    std::thread::spawn(move || {
        tracing::info_span!("task", id = %id)
            .follows_from(span)
            .in_scope(move || {
                // Ignore errors. Not much we can do and task::execute already does enough logging.
                let _ = task::execute(&request);
            });
    });

    ApiResponse::success(TaskSubmitResponse { id })
}

pub fn create_app() -> Router<()> {
    Router::new()
        .route("/task", post(task_submit))
        .layer(TraceLayer::new_for_http())
}

#[tokio::main(flavor = "current_thread")]
pub async fn start(address: &SocketAddr) {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .init();

    let app = create_app();
    let listener = tokio::net::TcpListener::bind(address).await.unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
