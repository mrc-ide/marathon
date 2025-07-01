use crate::messages::{
    QueuePullResponse, QueuePullResponseInner, QueueUpdateRequest, TaskSubmitResponse,
};
use crate::responses::{ApiResponse, ApiResponseExt};
use crate::task::{TaskId, TaskInfo, TaskRequest, TaskStatus};
use crate::Database;
use axum::extract::{Path, State};
use axum::http;
use axum::response::IntoResponseParts;
use axum::{routing::get, routing::post, Json, Router};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::net::ToSocketAddrs;
use tower_http::trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer};

#[derive(Clone, Debug)]
pub struct Configuration {
    /// The interval at which workers should poll for more work.
    ///
    /// This value will be returned as a Retry-After header used by workers.
    pub polling_interval: Duration,
}

impl Default for Configuration {
    fn default() -> Self {
        Configuration {
            polling_interval: Duration::from_secs(5),
        }
    }
}

#[derive(Clone)]
struct AppState {
    database: Database,
    config: Configuration,
}

async fn task_submit(
    State(state): State<AppState>,
    Json(request): Json<TaskRequest>,
) -> ApiResponse<TaskSubmitResponse> {
    let id = state.database.task_create(&request).await?;
    ApiResponse::created(TaskSubmitResponse { id })
}

async fn task_get(Path(id): Path<TaskId>, State(state): State<AppState>) -> ApiResponse<TaskInfo> {
    if let Some(task) = state.database.task_get_info(id).await? {
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
    ApiResponse::bad_request()
}

async fn queue_pull(
    State(state): State<AppState>,
) -> ApiResponse<QueuePullResponse, impl IntoResponseParts> {
    if let Some(task) = state.database.queue_pop().await? {
        let request = state.database.task_get_request(task).await?;
        ApiResponse::success(Some(QueuePullResponseInner { task, request })).with(None)
    } else {
        let interval = state.config.polling_interval.as_secs();
        ApiResponse::success(None).with(Some([(http::header::RETRY_AFTER, interval)]))
    }
}

async fn queue_update(
    State(state): State<AppState>,
    Json(body): Json<QueueUpdateRequest>,
) -> ApiResponse<()> {
    match body.status {
        TaskStatus::Running | TaskStatus::Complete | TaskStatus::Failed => (),
        _ => return ApiResponse::bad_request(),
    }

    state.database.task_update(body.task, body.status).await?;

    ApiResponse::success(())
}

pub struct ApiServer {
    router: Router<()>,
}

impl ApiServer {
    #[tracing::instrument(name = "setup", skip_all)]
    pub async fn new(config: Configuration) -> crate::Result<ApiServer> {
        let state = AppState {
            database: Database::open_in_memory().await.unwrap(),
            config,
        };

        // These are DEBUG level by default, which we disable on external crates.
        // Alternatively we could keep them at debug but ensure they are attributed to this crate so
        // they would be logged.
        let trace = TraceLayer::new_for_http()
            .make_span_with(DefaultMakeSpan::new().level(tracing::Level::INFO))
            .on_request(DefaultOnRequest::new().level(tracing::Level::INFO))
            .on_response(DefaultOnResponse::new().level(tracing::Level::INFO));

        let router = Router::new()
            .route("/", get(index))
            .route("/tasks", get(task_list).post(task_submit))
            .route("/tasks/{id}", get(task_get))
            .route("/queue/pull", post(queue_pull))
            .route("/queue/task", post(queue_update))
            .fallback(fallback_handler)
            .with_state(state)
            .layer(trace);

        Ok(ApiServer { router })
    }

    /// Start listening on the given address.
    ///
    /// This may be called multiple times to listen on multiple addresses.
    /// The returned future must be awaited in order to process requests.
    pub async fn listen(
        &self,
        addr: impl ToSocketAddrs,
    ) -> crate::Result<axum::serve::Serve<TcpListener, Router, Router>> {
        let listener = TcpListener::bind(addr).await?;
        let addr = listener.local_addr()?;

        tracing::info!("listening on {addr}");

        Ok(axum::serve(listener, self.router.clone()))
    }
}
