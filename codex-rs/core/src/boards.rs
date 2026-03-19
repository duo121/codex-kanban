use anyhow::Result;
use codex_protocol::ThreadId;
use std::sync::Arc;

use crate::config::Config;
use crate::find_thread_path_by_id_str;

pub use codex_state::Board;
pub use codex_state::BoardOverview;
pub use codex_state::BoardSession;
pub use codex_state::BoardSessionMoveDirection;
pub use codex_state::BoardSessionStatus;

#[derive(Clone)]
pub struct BoardRegistry {
    codex_home: std::path::PathBuf,
    runtime: Arc<codex_state::StateRuntime>,
}

impl BoardRegistry {
    pub async fn open(config: &Config) -> Result<Self> {
        let runtime = codex_state::StateRuntime::init(
            config.sqlite_home.clone(),
            config.model_provider_id.to_string(),
        )
        .await?;
        Ok(Self {
            codex_home: config.codex_home.clone(),
            runtime,
        })
    }

    pub async fn list_boards(&self) -> Result<Vec<BoardOverview>> {
        self.runtime.list_boards().await
    }

    pub async fn get_board(&self, board_id: &str) -> Result<Option<Board>> {
        self.runtime.get_board(board_id).await
    }

    pub async fn create_board(&self, name: &str) -> Result<Board> {
        self.runtime.create_board(name).await
    }

    pub async fn rename_board(&self, board_id: &str, name: &str) -> Result<bool> {
        self.runtime.rename_board(board_id, name).await
    }

    pub async fn delete_board(&self, board_id: &str) -> Result<bool> {
        self.runtime.delete_board(board_id).await
    }

    pub async fn set_board_last_selected_thread(
        &self,
        board_id: &str,
        thread_id: Option<ThreadId>,
    ) -> Result<bool> {
        self.runtime
            .set_board_last_selected_thread(board_id, thread_id)
            .await
    }

    pub async fn list_board_sessions(&self, board_id: &str) -> Result<Vec<BoardSession>> {
        self.runtime.list_board_sessions(board_id).await
    }

    pub async fn add_board_session(
        &self,
        board_id: &str,
        thread_id: ThreadId,
        title_snapshot: &str,
    ) -> Result<()> {
        self.runtime
            .add_board_session(board_id, thread_id, title_snapshot)
            .await
    }

    pub async fn remove_board_session(&self, board_id: &str, thread_id: ThreadId) -> Result<bool> {
        self.runtime.remove_board_session(board_id, thread_id).await
    }

    pub async fn replace_board_session_thread_id(
        &self,
        board_id: &str,
        old_thread_id: ThreadId,
        new_thread_id: ThreadId,
        title_snapshot: &str,
    ) -> Result<bool> {
        self.runtime
            .replace_board_session_thread_id(board_id, old_thread_id, new_thread_id, title_snapshot)
            .await
    }

    pub async fn update_board_session_title_snapshot(
        &self,
        board_id: &str,
        thread_id: ThreadId,
        title_snapshot: &str,
    ) -> Result<bool> {
        self.runtime
            .update_board_session_title_snapshot(board_id, thread_id, title_snapshot)
            .await
    }

    pub async fn update_thread_board_session_title_snapshot(
        &self,
        thread_id: ThreadId,
        title_snapshot: &str,
    ) -> Result<u64> {
        self.runtime
            .update_thread_board_session_title_snapshot(thread_id, title_snapshot)
            .await
    }

    pub async fn set_board_session_status(
        &self,
        board_id: &str,
        thread_id: ThreadId,
        status: BoardSessionStatus,
    ) -> Result<bool> {
        self.runtime
            .set_board_session_status(board_id, thread_id, status)
            .await
    }

    pub async fn set_thread_board_sessions_status(
        &self,
        thread_id: ThreadId,
        status: BoardSessionStatus,
    ) -> Result<u64> {
        self.runtime
            .set_thread_board_sessions_status(thread_id, status)
            .await
    }

    pub async fn record_thread_board_sessions_activity(
        &self,
        thread_id: ThreadId,
        status: BoardSessionStatus,
    ) -> Result<u64> {
        self.runtime
            .record_thread_board_sessions_activity(thread_id, status)
            .await
    }

    pub async fn mark_board_session_seen(
        &self,
        board_id: &str,
        thread_id: ThreadId,
    ) -> Result<bool> {
        self.runtime
            .mark_board_session_seen(board_id, thread_id)
            .await
    }

    pub async fn move_board_session(
        &self,
        board_id: &str,
        thread_id: ThreadId,
        direction: BoardSessionMoveDirection,
    ) -> Result<bool> {
        self.runtime
            .move_board_session(board_id, thread_id, direction)
            .await
    }

    pub async fn find_rollout_path_by_id(&self, thread_id: ThreadId) -> Option<std::path::PathBuf> {
        if let Some(path) = self
            .runtime
            .find_rollout_path_by_id(thread_id, /*archived_only*/ Some(false))
            .await
            .ok()
            .flatten()
        {
            return Some(path);
        }

        find_thread_path_by_id_str(self.codex_home.as_path(), &thread_id.to_string())
            .await
            .ok()
            .flatten()
    }
}
