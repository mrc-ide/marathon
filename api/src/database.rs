use crate::task::{TaskId, TaskInfo, TaskRequest, TaskStatus};
use anyhow::bail;
use sqlx::types::Json;
use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn open_in_memory() -> crate::Result<Database> {
        let pool = SqlitePool::connect("sqlite::memory:").await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Database { pool })
    }

    /// Create a new task and add it to the queue.
    ///
    /// The task request is serialized and stored in the database. The task's initial status will
    /// be `Pending`. A new unique identifier is generated for this task and is returned.
    pub async fn task_create(&self, req: &TaskRequest) -> crate::Result<TaskId> {
        let id = TaskId::new();

        let mut tx = self.pool.begin().await?;

        sqlx::query("INSERT INTO task (id, status, request) VALUES ($1, $2, jsonb($3))")
            .bind(id)
            .bind(TaskStatus::Pending)
            .bind(serde_json::to_string(req)?)
            .execute(&mut *tx)
            .await?;

        sqlx::query("INSERT INTO task_queue (task) VALUES ($1)")
            .bind(id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        Ok(id)
    }

    pub async fn task_get_info(&self, id: TaskId) -> crate::Result<Option<TaskInfo>> {
        let task = sqlx::query_as::<_, TaskInfo>("SELECT id, status FROM task WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(task)
    }

    pub async fn task_get_request(&self, id: TaskId) -> crate::Result<TaskRequest> {
        let Json(request): Json<TaskRequest> =
            sqlx::query_scalar("SELECT json(request) FROM task WHERE id = $1")
                .bind(id)
                .fetch_one(&self.pool)
                .await?;
        Ok(request)
    }

    pub async fn task_list(&self) -> crate::Result<Vec<TaskInfo>> {
        let tasks = sqlx::query_as::<_, TaskInfo>("SELECT id, status FROM task")
            .fetch_all(&self.pool)
            .await?;
        Ok(tasks)
    }

    pub async fn task_update(&self, id: TaskId, status: TaskStatus) -> crate::Result<()> {
        tracing::debug!(task = %id, ?status, "updating task status");
        let result = sqlx::query("UPDATE task SET status = $1 WHERE id = $2")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            bail!("invalid task");
        }
        Ok(())
    }

    /// Remove the first task from the work queue and mark its status as "Claimed".
    pub async fn queue_pop(&self) -> crate::Result<Option<TaskId>> {
        tracing::debug!("pulling a task from the work queue");

        let mut tx = self.pool.begin().await?;

        let task = sqlx::query_scalar(
            "DELETE FROM task_queue
             WHERE seq = (SELECT seq FROM task_queue ORDER BY seq LIMIT 1)
             RETURNING task",
        )
        .fetch_optional(&mut *tx)
        .await?;

        sqlx::query("UPDATE task SET status = $1 WHERE id = $2")
            .bind(TaskStatus::Claimed)
            .bind(task)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        Ok(task)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assertor::*;
    use std::collections::HashMap;

    fn dummy_request() -> TaskRequest {
        TaskRequest {
            cmdline: vec!["hello".to_owned()],
            environment: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn can_create_database() {
        assert_that!(Database::open_in_memory().await).is_ok();
    }

    #[tokio::test]
    async fn can_create_task() -> anyhow::Result<()> {
        let db = Database::open_in_memory().await?;
        let id = db.task_create(&dummy_request()).await?;
        let task = db.task_get_info(id).await?;
        assert_that!(task).some().is_equal_to(TaskInfo {
            id,
            status: TaskStatus::Pending,
        });

        Ok(())
    }

    #[tokio::test]
    async fn can_pop_tasks_from_queue() -> anyhow::Result<()> {
        let db = Database::open_in_memory().await?;

        let mut ids = vec![];
        ids.push(db.task_create(&dummy_request()).await?);
        ids.push(db.task_create(&dummy_request()).await?);
        ids.push(db.task_create(&dummy_request()).await?);

        assert_that!(db.task_get_info(ids[0]).await?.map(|t| t.status))
            .some()
            .is_equal_to(TaskStatus::Pending);

        let mut result = vec![];
        result.push(db.queue_pop().await?);
        result.push(db.queue_pop().await?);
        result.push(db.queue_pop().await?);
        result.push(db.queue_pop().await?);

        assert_that!(db.task_get_info(ids[0]).await?.map(|t| t.status))
            .some()
            .is_equal_to(TaskStatus::Claimed);

        assert_eq!(result[..3], ids.into_iter().map(Some).collect::<Vec<_>>());
        assert_that!(result[3]).is_none();

        Ok(())
    }

    #[tokio::test]
    async fn returns_none_on_missing_task() -> anyhow::Result<()> {
        let db = Database::open_in_memory().await?;
        let id = TaskId::new();

        assert_that!(db.task_get_info(id).await?).is_none();

        Ok(())
    }

    #[tokio::test]
    async fn can_update_task_status() -> anyhow::Result<()> {
        let db = Database::open_in_memory().await?;
        let id = db.task_create(&dummy_request()).await?;

        db.task_update(id, TaskStatus::Running).await?;

        let task = db.task_get_info(id).await?;
        assert_that!(task).some().is_equal_to(TaskInfo {
            id,
            status: TaskStatus::Running,
        });

        Ok(())
    }

    #[tokio::test]
    async fn updating_invalid_task_fails() -> anyhow::Result<()> {
        let db = Database::open_in_memory().await?;
        let id = TaskId::new();

        let result = db.task_update(id, TaskStatus::Running).await;
        assert_that!(result).err().has_message("invalid task");

        Ok(())
    }

    #[tokio::test]
    async fn can_list_tasks() -> anyhow::Result<()> {
        let db = Database::open_in_memory().await?;
        let id1 = db.task_create(&dummy_request()).await?;
        let id2 = db.task_create(&dummy_request()).await?;
        db.task_update(id1, TaskStatus::Running).await?;

        let tasks = db.task_list().await?;
        assert_that!(tasks).contains_exactly(vec![
            TaskInfo {
                id: id1,
                status: TaskStatus::Running,
            },
            TaskInfo {
                id: id2,
                status: TaskStatus::Pending,
            },
        ]);

        Ok(())
    }
}
