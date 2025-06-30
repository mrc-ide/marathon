use crate::task::{TaskInfo, TaskStatus};
use anyhow::bail;
use sqlx::SqlitePool;
use uuid::Uuid;

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

    pub async fn task_create(&self) -> crate::Result<Uuid> {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO task (id, status) VALUES ($1, $2)")
            .bind(id)
            .bind(TaskStatus::Pending)
            .execute(&self.pool)
            .await?;
        Ok(id)
    }

    pub async fn task_get_status(&self, id: Uuid) -> crate::Result<Option<TaskInfo>> {
        let task = sqlx::query_as::<_, TaskInfo>("SELECT id, status FROM task WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(task)
    }

    pub async fn task_list(&self) -> crate::Result<Vec<TaskInfo>> {
        let tasks = sqlx::query_as::<_, TaskInfo>("SELECT id, status FROM task")
            .fetch_all(&self.pool)
            .await?;
        Ok(tasks)
    }

    pub async fn task_update(&self, id: Uuid, status: TaskStatus) -> crate::Result<()> {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use assertor::*;

    #[tokio::test]
    async fn can_create_database() {
        assert_that!(Database::open_in_memory().await).is_ok();
    }

    #[tokio::test]
    async fn can_create_task() -> anyhow::Result<()> {
        let db = Database::open_in_memory().await?;
        let id = db.task_create().await?;

        let task = db.task_get_status(id).await?;
        assert_that!(task).some().is_equal_to(TaskInfo {
            id,
            status: TaskStatus::Pending,
        });

        Ok(())
    }

    #[tokio::test]
    async fn returns_none_on_missing_task() -> anyhow::Result<()> {
        let db = Database::open_in_memory().await?;
        let id = Uuid::new_v4();

        assert_that!(db.task_get_status(id).await?).is_none();

        Ok(())
    }

    #[tokio::test]
    async fn can_update_task_status() -> anyhow::Result<()> {
        let db = Database::open_in_memory().await?;
        let id = db.task_create().await?;

        db.task_update(id, TaskStatus::Running).await?;

        let task = db.task_get_status(id).await?;
        assert_that!(task).some().is_equal_to(TaskInfo {
            id,
            status: TaskStatus::Running,
        });

        Ok(())
    }

    #[tokio::test]
    async fn updating_invalid_task_fails() -> anyhow::Result<()> {
        let db = Database::open_in_memory().await?;
        let id = Uuid::new_v4();

        let result = db.task_update(id, TaskStatus::Running).await;
        assert_that!(result).err().has_message("invalid task");

        Ok(())
    }

    #[tokio::test]
    async fn can_list_tasks() -> anyhow::Result<()> {
        let db = Database::open_in_memory().await?;
        let id1 = db.task_create().await?;
        let id2 = db.task_create().await?;
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
