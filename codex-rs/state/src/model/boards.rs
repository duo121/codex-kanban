use anyhow::Result;
use chrono::DateTime;
use chrono::Utc;
use codex_protocol::ThreadId;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use crate::model::thread_metadata::datetime_to_epoch_seconds;
use crate::model::thread_metadata::epoch_seconds_to_datetime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardSessionStatus {
    Unknown,
    Running,
    NeedsAttention,
    Seen,
    WaitingApproval,
    Errored,
}

impl BoardSessionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Running => "running",
            Self::NeedsAttention => "needs_attention",
            Self::Seen => "seen",
            Self::WaitingApproval => "waiting_approval",
            Self::Errored => "errored",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardSessionMoveDirection {
    Up,
    Down,
}

impl TryFrom<&str> for BoardSessionStatus {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        match value {
            "unknown" => Ok(Self::Unknown),
            "running" => Ok(Self::Running),
            "needs_attention" => Ok(Self::NeedsAttention),
            "seen" => Ok(Self::Seen),
            "waiting_approval" => Ok(Self::WaitingApproval),
            "errored" => Ok(Self::Errored),
            other => Err(anyhow::anyhow!("invalid board session status: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    pub id: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_selected_thread_id: Option<ThreadId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardOverview {
    pub board: Board,
    pub session_count: usize,
    pub running_count: usize,
    pub needs_attention_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardSession {
    pub board_id: String,
    pub thread_id: ThreadId,
    pub title_snapshot: String,
    pub sort_order: i64,
    pub status: BoardSessionStatus,
    pub last_seen_event_idx: Option<i64>,
    pub last_event_idx: Option<i64>,
    pub added_at: DateTime<Utc>,
    pub removed_at: Option<DateTime<Utc>>,
}

#[derive(Debug)]
pub(crate) struct BoardRow {
    id: String,
    name: String,
    created_at: i64,
    updated_at: i64,
    last_selected_thread_id: Option<String>,
}

impl BoardRow {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
            last_selected_thread_id: row.try_get("last_selected_thread_id")?,
        })
    }
}

impl TryFrom<BoardRow> for Board {
    type Error = anyhow::Error;

    fn try_from(row: BoardRow) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            name: row.name,
            created_at: epoch_seconds_to_datetime(row.created_at)?,
            updated_at: epoch_seconds_to_datetime(row.updated_at)?,
            last_selected_thread_id: row
                .last_selected_thread_id
                .map(ThreadId::try_from)
                .transpose()?,
        })
    }
}

#[derive(Debug)]
pub(crate) struct BoardOverviewRow {
    board: BoardRow,
    session_count: i64,
    running_count: i64,
    needs_attention_count: i64,
}

impl BoardOverviewRow {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            board: BoardRow::try_from_row(row)?,
            session_count: row.try_get("session_count")?,
            running_count: row.try_get("running_count")?,
            needs_attention_count: row.try_get("needs_attention_count")?,
        })
    }
}

impl TryFrom<BoardOverviewRow> for BoardOverview {
    type Error = anyhow::Error;

    fn try_from(row: BoardOverviewRow) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            board: row.board.try_into()?,
            session_count: usize::try_from(row.session_count)?,
            running_count: usize::try_from(row.running_count)?,
            needs_attention_count: usize::try_from(row.needs_attention_count)?,
        })
    }
}

#[derive(Debug)]
pub(crate) struct BoardSessionRow {
    board_id: String,
    thread_id: String,
    title_snapshot: String,
    sort_order: i64,
    status: String,
    last_seen_event_idx: Option<i64>,
    last_event_idx: Option<i64>,
    added_at: i64,
    removed_at: Option<i64>,
}

impl BoardSessionRow {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            board_id: row.try_get("board_id")?,
            thread_id: row.try_get("thread_id")?,
            title_snapshot: row.try_get("title_snapshot")?,
            sort_order: row.try_get("sort_order")?,
            status: row.try_get("status")?,
            last_seen_event_idx: row.try_get("last_seen_event_idx")?,
            last_event_idx: row.try_get("last_event_idx")?,
            added_at: row.try_get("added_at")?,
            removed_at: row.try_get("removed_at")?,
        })
    }
}

impl TryFrom<BoardSessionRow> for BoardSession {
    type Error = anyhow::Error;

    fn try_from(row: BoardSessionRow) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            board_id: row.board_id,
            thread_id: ThreadId::try_from(row.thread_id)?,
            title_snapshot: row.title_snapshot,
            sort_order: row.sort_order,
            status: BoardSessionStatus::try_from(row.status.as_str())?,
            last_seen_event_idx: row.last_seen_event_idx,
            last_event_idx: row.last_event_idx,
            added_at: epoch_seconds_to_datetime(row.added_at)?,
            removed_at: row.removed_at.map(epoch_seconds_to_datetime).transpose()?,
        })
    }
}

pub(crate) fn now_epoch_seconds() -> i64 {
    datetime_to_epoch_seconds(Utc::now())
}
