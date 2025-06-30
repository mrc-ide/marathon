use crate::messages::TaskSubmitResponse;
use crate::responses::{ApiResponse, ApiResponseExt};
use crate::task::{TaskResult, TaskStatus};
use crate::{execute, Database, TaskInfo, TaskRequest};
use axum::extract::{Path, State};
use axum::{routing::get, Json, Router};
use std::net::SocketAddr;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    database: Database,
}

/// This runs a marathon task in a background thread.
///
/// Eventually this will go away and be replaced by workers and a queue.
fn execute_thread(
    database: Database,
    handle: tokio::runtime::Handle,
    id: Uuid,
    request: TaskRequest,
) -> crate::Result<()> {
    let _guard = tracing::info_span!("task", id = %id).entered();

    handle.block_on(database.task_update(id, TaskStatus::Running))?;

    let status = match execute(&request) {
        Ok(TaskResult { status, .. }) => {
            if status.success() {
                TaskStatus::Complete
            } else {
                TaskStatus::Failed
            }
        }
        Err(_) => TaskStatus::Failed,
    };

    handle.block_on(database.task_update(id, status))?;

    Ok(())
}

async fn task_submit(
    State(state): State<AppState>,
    Json(request): Json<TaskRequest>,
) -> ApiResponse<TaskSubmitResponse> {
    let id = state.database.task_create().await?;

    // We need the handle to do database updates from the background thread.
    let handle = tokio::runtime::Handle::current();

    // Eventually this should add to a queue, rather than run immediately.
    std::thread::spawn(move || {
        if let Err(error) = execute_thread(state.database, handle, id, request) {
            // Not a lot more we can do about it at this point. This only concerns unexpected
            // errors (eg. updating the database). Errors caused by the actual task are caught and
            // recorded in the database.
            tracing::error!(%error, "error while processing task");
        }
    });

    ApiResponse::created(TaskSubmitResponse { id })
}

async fn task_get(Path(id): Path<Uuid>, State(state): State<AppState>) -> ApiResponse<TaskInfo> {
    if let Some(task) = state.database.task_get_status(id).await? {
        ApiResponse::success(task)
    } else {
        ApiResponse::not_found()
    }
}

async fn task_list(State(state): State<AppState>) -> ApiResponse<Vec<TaskInfo>> {
    let tasks = state.database.task_list().await?;
    ApiResponse::success(tasks)
}

async fn index() -> ApiResponse<()> {
    ApiResponse::success(())
}

async fn fallback_handler() -> ApiResponse<()> {
    ApiResponse::not_found()
}

pub async fn create_app() -> crate::Result<Router<()>> {
    let state = AppState {
        database: Database::open_in_memory().await.unwrap(),
    };

    Ok(Router::new()
        .route("/", get(index))
        .route("/tasks", get(task_list).post(task_submit))
        .route("/tasks/{id}", get(task_get))
        .fallback(fallback_handler)
        .with_state(state)
        .layer(TraceLayer::new_for_http()))
}

#[tokio::main(flavor = "current_thread")]
pub async fn start(address: &SocketAddr) -> crate::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .init();

    let app = create_app().await?;

    let listener = tokio::net::TcpListener::bind(address).await?;
    println!("listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;

    Ok(())
}
