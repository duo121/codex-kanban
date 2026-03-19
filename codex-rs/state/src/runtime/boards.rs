use super::*;
use crate::Board;
use crate::BoardOverview;
use crate::BoardSession;
use crate::BoardSessionMoveDirection;
use crate::BoardSessionStatus;
use crate::model::BoardOverviewRow;
use crate::model::BoardRow;
use crate::model::BoardSessionRow;
use crate::model::now_epoch_seconds;
use sqlx::Acquire;
use uuid::Uuid;

impl StateRuntime {
    pub async fn list_boards(&self) -> anyhow::Result<Vec<BoardOverview>> {
        let rows = sqlx::query(
            r#"
SELECT
    b.id,
    b.name,
    b.created_at,
    b.updated_at,
    b.last_selected_thread_id,
    COUNT(bs.thread_id) AS session_count,
    COALESCE(SUM(CASE WHEN bs.status = 'running' THEN 1 ELSE 0 END), 0) AS running_count,
    COALESCE(SUM(CASE WHEN bs.status = 'needs_attention' THEN 1 ELSE 0 END), 0) AS needs_attention_count
FROM boards b
LEFT JOIN board_sessions bs
    ON bs.board_id = b.id
    AND bs.removed_at IS NULL
GROUP BY b.id, b.name, b.created_at, b.updated_at, b.last_selected_thread_id
ORDER BY b.updated_at DESC, b.name ASC
            "#,
        )
        .fetch_all(self.pool.as_ref())
        .await?;
        rows.into_iter()
            .map(|row| BoardOverviewRow::try_from_row(&row).and_then(BoardOverview::try_from))
            .collect()
    }

    pub async fn get_board(&self, board_id: &str) -> anyhow::Result<Option<Board>> {
        let row = sqlx::query(
            r#"
SELECT id, name, created_at, updated_at, last_selected_thread_id
FROM boards
WHERE id = ?
            "#,
        )
        .bind(board_id)
        .fetch_optional(self.pool.as_ref())
        .await?;
        row.map(|row| BoardRow::try_from_row(&row).and_then(Board::try_from))
            .transpose()
    }

    pub async fn create_board(&self, name: &str) -> anyhow::Result<Board> {
        let id = Uuid::new_v4().to_string();
        let now = now_epoch_seconds();
        sqlx::query(
            r#"
INSERT INTO boards (id, name, created_at, updated_at, last_selected_thread_id)
VALUES (?, ?, ?, ?, NULL)
            "#,
        )
        .bind(id.as_str())
        .bind(name)
        .bind(now)
        .bind(now)
        .execute(self.pool.as_ref())
        .await?;
        self.get_board(id.as_str())
            .await?
            .ok_or_else(|| anyhow::anyhow!("created board is missing"))
    }

    pub async fn rename_board(&self, board_id: &str, name: &str) -> anyhow::Result<bool> {
        let result = sqlx::query(
            r#"
UPDATE boards
SET name = ?, updated_at = ?
WHERE id = ?
            "#,
        )
        .bind(name)
        .bind(now_epoch_seconds())
        .bind(board_id)
        .execute(self.pool.as_ref())
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn delete_board(&self, board_id: &str) -> anyhow::Result<bool> {
        let result = sqlx::query("DELETE FROM boards WHERE id = ?")
            .bind(board_id)
            .execute(self.pool.as_ref())
            .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn set_board_last_selected_thread(
        &self,
        board_id: &str,
        thread_id: Option<ThreadId>,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            r#"
UPDATE boards
SET last_selected_thread_id = ?, updated_at = ?
WHERE id = ?
            "#,
        )
        .bind(thread_id.map(|thread_id| thread_id.to_string()))
        .bind(now_epoch_seconds())
        .bind(board_id)
        .execute(self.pool.as_ref())
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn list_board_sessions(&self, board_id: &str) -> anyhow::Result<Vec<BoardSession>> {
        let rows = sqlx::query(
            r#"
SELECT
    board_id,
    thread_id,
    title_snapshot,
    sort_order,
    status,
    last_seen_event_idx,
    last_event_idx,
    added_at,
    removed_at
FROM board_sessions
WHERE board_id = ?
  AND removed_at IS NULL
ORDER BY sort_order ASC, added_at ASC
            "#,
        )
        .bind(board_id)
        .fetch_all(self.pool.as_ref())
        .await?;
        rows.into_iter()
            .map(|row| BoardSessionRow::try_from_row(&row).and_then(BoardSession::try_from))
            .collect()
    }

    pub async fn add_board_session(
        &self,
        board_id: &str,
        thread_id: ThreadId,
        title_snapshot: &str,
    ) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        let conn = tx.acquire().await?;
        let next_sort_order = sqlx::query_scalar::<Sqlite, i64>(
            r#"
SELECT COALESCE(MAX(sort_order), -1) + 1
FROM board_sessions
WHERE board_id = ?
  AND removed_at IS NULL
            "#,
        )
        .bind(board_id)
        .fetch_one(&mut *conn)
        .await?;
        let now = now_epoch_seconds();
        sqlx::query(
            r#"
INSERT INTO board_sessions (
    board_id,
    thread_id,
    title_snapshot,
    sort_order,
    status,
    last_seen_event_idx,
    last_event_idx,
    added_at,
    removed_at
) VALUES (?, ?, ?, ?, ?, NULL, NULL, ?, NULL)
ON CONFLICT(board_id, thread_id) DO UPDATE SET
    title_snapshot = excluded.title_snapshot,
    sort_order = excluded.sort_order,
    status = excluded.status,
    added_at = excluded.added_at,
    removed_at = NULL
            "#,
        )
        .bind(board_id)
        .bind(thread_id.to_string())
        .bind(title_snapshot)
        .bind(next_sort_order)
        .bind(BoardSessionStatus::Unknown.as_str())
        .bind(now)
        .execute(&mut *conn)
        .await?;
        sqlx::query(
            r#"
UPDATE boards
SET updated_at = ?, last_selected_thread_id = ?
WHERE id = ?
            "#,
        )
        .bind(now)
        .bind(thread_id.to_string())
        .bind(board_id)
        .execute(&mut *conn)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn remove_board_session(
        &self,
        board_id: &str,
        thread_id: ThreadId,
    ) -> anyhow::Result<bool> {
        let mut tx = self.pool.begin().await?;
        let conn = tx.acquire().await?;
        let now = now_epoch_seconds();
        let result = sqlx::query(
            r#"
UPDATE board_sessions
SET removed_at = ?
WHERE board_id = ?
  AND thread_id = ?
  AND removed_at IS NULL
            "#,
        )
        .bind(now)
        .bind(board_id)
        .bind(thread_id.to_string())
        .execute(&mut *conn)
        .await?;
        if result.rows_affected() == 1 {
            sqlx::query("UPDATE boards SET updated_at = ? WHERE id = ?")
                .bind(now)
                .bind(board_id)
                .execute(&mut *conn)
                .await?;
        }
        tx.commit().await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn replace_board_session_thread_id(
        &self,
        board_id: &str,
        old_thread_id: ThreadId,
        new_thread_id: ThreadId,
        title_snapshot: &str,
    ) -> anyhow::Result<bool> {
        let mut tx = self.pool.begin().await?;
        let conn = tx.acquire().await?;
        let now = now_epoch_seconds();
        let result = sqlx::query(
            r#"
UPDATE board_sessions
SET thread_id = ?,
    title_snapshot = ?,
    status = ?,
    last_seen_event_idx = NULL,
    last_event_idx = NULL
WHERE board_id = ?
  AND thread_id = ?
  AND removed_at IS NULL
            "#,
        )
        .bind(new_thread_id.to_string())
        .bind(title_snapshot)
        .bind(BoardSessionStatus::Unknown.as_str())
        .bind(board_id)
        .bind(old_thread_id.to_string())
        .execute(&mut *conn)
        .await?;
        if result.rows_affected() == 1 {
            sqlx::query(
                r#"
UPDATE boards
SET updated_at = ?,
    last_selected_thread_id = CASE
        WHEN last_selected_thread_id = ? THEN ?
        ELSE last_selected_thread_id
    END
WHERE id = ?
                "#,
            )
            .bind(now)
            .bind(old_thread_id.to_string())
            .bind(new_thread_id.to_string())
            .bind(board_id)
            .execute(&mut *conn)
            .await?;
        }
        tx.commit().await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn update_board_session_title_snapshot(
        &self,
        board_id: &str,
        thread_id: ThreadId,
        title_snapshot: &str,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            r#"
UPDATE board_sessions
SET title_snapshot = ?
WHERE board_id = ?
  AND thread_id = ?
  AND removed_at IS NULL
            "#,
        )
        .bind(title_snapshot)
        .bind(board_id)
        .bind(thread_id.to_string())
        .execute(self.pool.as_ref())
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn update_thread_board_session_title_snapshot(
        &self,
        thread_id: ThreadId,
        title_snapshot: &str,
    ) -> anyhow::Result<u64> {
        let mut tx = self.pool.begin().await?;
        let conn = tx.acquire().await?;
        let now = now_epoch_seconds();
        let result = sqlx::query(
            r#"
UPDATE board_sessions
SET title_snapshot = ?
WHERE thread_id = ?
  AND removed_at IS NULL
            "#,
        )
        .bind(title_snapshot)
        .bind(thread_id.to_string())
        .execute(&mut *conn)
        .await?;
        if result.rows_affected() > 0 {
            sqlx::query(
                r#"
UPDATE boards
SET updated_at = ?
WHERE id IN (
    SELECT board_id
    FROM board_sessions
    WHERE thread_id = ?
      AND removed_at IS NULL
)
                "#,
            )
            .bind(now)
            .bind(thread_id.to_string())
            .execute(&mut *conn)
            .await?;
        }
        tx.commit().await?;
        Ok(result.rows_affected())
    }

    pub async fn set_board_session_status(
        &self,
        board_id: &str,
        thread_id: ThreadId,
        status: BoardSessionStatus,
    ) -> anyhow::Result<bool> {
        let mut tx = self.pool.begin().await?;
        let conn = tx.acquire().await?;
        let now = now_epoch_seconds();
        let result = sqlx::query(
            r#"
UPDATE board_sessions
SET status = ?
WHERE board_id = ?
  AND thread_id = ?
  AND removed_at IS NULL
            "#,
        )
        .bind(status.as_str())
        .bind(board_id)
        .bind(thread_id.to_string())
        .execute(&mut *conn)
        .await?;
        if result.rows_affected() == 1 {
            sqlx::query("UPDATE boards SET updated_at = ? WHERE id = ?")
                .bind(now)
                .bind(board_id)
                .execute(&mut *conn)
                .await?;
        }
        tx.commit().await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn set_thread_board_sessions_status(
        &self,
        thread_id: ThreadId,
        status: BoardSessionStatus,
    ) -> anyhow::Result<u64> {
        let mut tx = self.pool.begin().await?;
        let conn = tx.acquire().await?;
        let now = now_epoch_seconds();
        let result = sqlx::query(
            r#"
UPDATE board_sessions
SET status = ?
WHERE thread_id = ?
  AND removed_at IS NULL
            "#,
        )
        .bind(status.as_str())
        .bind(thread_id.to_string())
        .execute(&mut *conn)
        .await?;
        if result.rows_affected() > 0 {
            sqlx::query(
                r#"
UPDATE boards
SET updated_at = ?
WHERE id IN (
    SELECT board_id
    FROM board_sessions
    WHERE thread_id = ?
      AND removed_at IS NULL
)
                "#,
            )
            .bind(now)
            .bind(thread_id.to_string())
            .execute(&mut *conn)
            .await?;
        }
        tx.commit().await?;
        Ok(result.rows_affected())
    }

    pub async fn record_thread_board_sessions_activity(
        &self,
        thread_id: ThreadId,
        status: BoardSessionStatus,
    ) -> anyhow::Result<u64> {
        let mut tx = self.pool.begin().await?;
        let conn = tx.acquire().await?;
        let now = now_epoch_seconds();
        let result = sqlx::query(
            r#"
UPDATE board_sessions
SET
    status = ?,
    last_event_idx = COALESCE(last_event_idx, -1) + 1
WHERE thread_id = ?
  AND removed_at IS NULL
            "#,
        )
        .bind(status.as_str())
        .bind(thread_id.to_string())
        .execute(&mut *conn)
        .await?;
        if result.rows_affected() > 0 {
            sqlx::query(
                r#"
UPDATE boards
SET updated_at = ?
WHERE id IN (
    SELECT board_id
    FROM board_sessions
    WHERE thread_id = ?
      AND removed_at IS NULL
)
                "#,
            )
            .bind(now)
            .bind(thread_id.to_string())
            .execute(&mut *conn)
            .await?;
        }
        tx.commit().await?;
        Ok(result.rows_affected())
    }

    pub async fn mark_board_session_seen(
        &self,
        board_id: &str,
        thread_id: ThreadId,
    ) -> anyhow::Result<bool> {
        let mut tx = self.pool.begin().await?;
        let conn = tx.acquire().await?;
        let now = now_epoch_seconds();
        let result = sqlx::query(
            r#"
UPDATE board_sessions
SET
    status = CASE
        WHEN status IN ('running', 'waiting_approval', 'errored') THEN status
        ELSE ?
    END,
    last_seen_event_idx = CASE
        WHEN last_event_idx IS NULL THEN last_seen_event_idx
        ELSE last_event_idx
    END
WHERE board_id = ?
  AND thread_id = ?
  AND removed_at IS NULL
            "#,
        )
        .bind(BoardSessionStatus::Seen.as_str())
        .bind(board_id)
        .bind(thread_id.to_string())
        .execute(&mut *conn)
        .await?;
        if result.rows_affected() == 1 {
            sqlx::query("UPDATE boards SET updated_at = ? WHERE id = ?")
                .bind(now)
                .bind(board_id)
                .execute(&mut *conn)
                .await?;
        }
        tx.commit().await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn move_board_session(
        &self,
        board_id: &str,
        thread_id: ThreadId,
        direction: BoardSessionMoveDirection,
    ) -> anyhow::Result<bool> {
        let mut tx = self.pool.begin().await?;
        let conn = tx.acquire().await?;
        let mut ordered_thread_ids = sqlx::query_scalar::<Sqlite, String>(
            r#"
SELECT thread_id
FROM board_sessions
WHERE board_id = ?
  AND removed_at IS NULL
ORDER BY sort_order ASC, added_at ASC
            "#,
        )
        .bind(board_id)
        .fetch_all(&mut *conn)
        .await?;
        let thread_id = thread_id.to_string();
        let Some(selected_idx) = ordered_thread_ids
            .iter()
            .position(|candidate| candidate == &thread_id)
        else {
            tx.commit().await?;
            return Ok(false);
        };
        let target_idx = match direction {
            BoardSessionMoveDirection::Up => selected_idx.checked_sub(1),
            BoardSessionMoveDirection::Down => {
                (selected_idx + 1 < ordered_thread_ids.len()).then_some(selected_idx + 1)
            }
        };
        let Some(target_idx) = target_idx else {
            tx.commit().await?;
            return Ok(false);
        };

        ordered_thread_ids.swap(selected_idx, target_idx);
        for (sort_order, listed_thread_id) in ordered_thread_ids.iter().enumerate() {
            sqlx::query(
                r#"
UPDATE board_sessions
SET sort_order = ?
WHERE board_id = ?
  AND thread_id = ?
  AND removed_at IS NULL
                "#,
            )
            .bind(i64::try_from(sort_order)?)
            .bind(board_id)
            .bind(listed_thread_id)
            .execute(&mut *conn)
            .await?;
        }
        sqlx::query("UPDATE boards SET updated_at = ? WHERE id = ?")
            .bind(now_epoch_seconds())
            .bind(board_id)
            .execute(&mut *conn)
            .await?;
        tx.commit().await?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::StateRuntime;
    use crate::BoardSessionMoveDirection;
    use crate::BoardSessionStatus;
    use crate::runtime::test_support::test_thread_metadata;
    use crate::runtime::test_support::unique_temp_dir;
    use codex_protocol::ThreadId;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn boards_round_trip_and_persist_sessions() -> anyhow::Result<()> {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string()).await?;

        let board = runtime.create_board("feat: payments").await?;
        assert_eq!(board.name, "feat: payments");

        let thread_id = ThreadId::new();
        let metadata = test_thread_metadata(
            codex_home.as_path(),
            thread_id,
            codex_home.join("workspace"),
        );
        runtime.upsert_thread(&metadata).await?;
        runtime
            .add_board_session(board.id.as_str(), thread_id, "Main implementation")
            .await?;
        runtime
            .set_board_last_selected_thread(board.id.as_str(), Some(thread_id))
            .await?;

        let boards = runtime.list_boards().await?;
        assert_eq!(boards.len(), 1);
        assert_eq!(boards[0].board.id, board.id);
        assert_eq!(boards[0].session_count, 1);
        assert_eq!(boards[0].running_count, 0);
        assert_eq!(boards[0].needs_attention_count, 0);

        let sessions = runtime.list_board_sessions(board.id.as_str()).await?;
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].thread_id, thread_id);
        assert_eq!(sessions[0].title_snapshot, "Main implementation");
        assert_eq!(sessions[0].status, BoardSessionStatus::Unknown);

        let _ = tokio::fs::remove_dir_all(codex_home).await;
        Ok(())
    }

    #[tokio::test]
    async fn removing_board_session_hides_it_from_active_list() -> anyhow::Result<()> {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string()).await?;

        let board = runtime.create_board("feat: auth").await?;
        let thread_id = ThreadId::new();
        runtime
            .add_board_session(board.id.as_str(), thread_id, "Debug login")
            .await?;
        assert_eq!(
            runtime.list_board_sessions(board.id.as_str()).await?.len(),
            1
        );

        let removed = runtime
            .remove_board_session(board.id.as_str(), thread_id)
            .await?;
        assert_eq!(removed, true);
        assert_eq!(
            runtime.list_board_sessions(board.id.as_str()).await?.len(),
            0
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
        Ok(())
    }

    #[tokio::test]
    async fn thread_activity_updates_all_board_rows_but_seen_is_board_specific()
    -> anyhow::Result<()> {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string()).await?;

        let board_a = runtime.create_board("feat: a").await?;
        let board_b = runtime.create_board("feat: b").await?;
        let thread_id = ThreadId::new();
        runtime
            .add_board_session(board_a.id.as_str(), thread_id, "Shared thread")
            .await?;
        runtime
            .add_board_session(board_b.id.as_str(), thread_id, "Shared thread")
            .await?;

        runtime
            .set_thread_board_sessions_status(thread_id, BoardSessionStatus::Running)
            .await?;
        runtime
            .record_thread_board_sessions_activity(thread_id, BoardSessionStatus::NeedsAttention)
            .await?;
        runtime
            .mark_board_session_seen(board_a.id.as_str(), thread_id)
            .await?;

        let board_a_sessions = runtime.list_board_sessions(board_a.id.as_str()).await?;
        let board_b_sessions = runtime.list_board_sessions(board_b.id.as_str()).await?;
        assert_eq!(board_a_sessions[0].status, BoardSessionStatus::Seen);
        assert_eq!(board_a_sessions[0].last_event_idx, Some(0));
        assert_eq!(board_a_sessions[0].last_seen_event_idx, Some(0));
        assert_eq!(
            board_b_sessions[0].status,
            BoardSessionStatus::NeedsAttention
        );
        assert_eq!(board_b_sessions[0].last_event_idx, Some(0));
        assert_eq!(board_b_sessions[0].last_seen_event_idx, None);

        let _ = tokio::fs::remove_dir_all(codex_home).await;
        Ok(())
    }

    #[tokio::test]
    async fn moving_board_session_reorders_board_list() -> anyhow::Result<()> {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string()).await?;

        let board = runtime.create_board("feat: reorder").await?;
        let thread_a = ThreadId::new();
        let thread_b = ThreadId::new();
        let thread_c = ThreadId::new();
        runtime
            .add_board_session(board.id.as_str(), thread_a, "A")
            .await?;
        runtime
            .add_board_session(board.id.as_str(), thread_b, "B")
            .await?;
        runtime
            .add_board_session(board.id.as_str(), thread_c, "C")
            .await?;

        assert_eq!(
            runtime
                .move_board_session(board.id.as_str(), thread_c, BoardSessionMoveDirection::Up,)
                .await?,
            true
        );
        assert_eq!(
            runtime
                .move_board_session(board.id.as_str(), thread_c, BoardSessionMoveDirection::Up,)
                .await?,
            true
        );

        let sessions = runtime.list_board_sessions(board.id.as_str()).await?;
        assert_eq!(
            sessions
                .into_iter()
                .map(|session| session.thread_id)
                .collect::<Vec<_>>(),
            vec![thread_c, thread_a, thread_b]
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
        Ok(())
    }

    #[tokio::test]
    async fn replacing_board_session_thread_id_preserves_slot_and_updates_selection()
    -> anyhow::Result<()> {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string()).await?;

        let board = runtime.create_board("feat: heal").await?;
        let old_thread_id = ThreadId::new();
        let newer_thread_id = ThreadId::new();
        runtime
            .add_board_session(board.id.as_str(), old_thread_id, "Untitled")
            .await?;
        runtime
            .set_board_last_selected_thread(board.id.as_str(), Some(old_thread_id))
            .await?;

        let replaced = runtime
            .replace_board_session_thread_id(
                board.id.as_str(),
                old_thread_id,
                newer_thread_id,
                "Recovered session",
            )
            .await?;
        assert_eq!(replaced, true);

        let board = runtime
            .get_board(board.id.as_str())
            .await?
            .expect("board should still exist");
        assert_eq!(board.last_selected_thread_id, Some(newer_thread_id));

        let sessions = runtime.list_board_sessions(board.id.as_str()).await?;
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].thread_id, newer_thread_id);
        assert_eq!(sessions[0].title_snapshot, "Recovered session");
        assert_eq!(sessions[0].status, BoardSessionStatus::Unknown);
        assert_eq!(sessions[0].sort_order, 0);

        let _ = tokio::fs::remove_dir_all(codex_home).await;
        Ok(())
    }
}
