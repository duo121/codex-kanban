use super::*;
use codex_core::boards::BoardSessionStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoardStatusUpdate {
    Set(BoardSessionStatus),
    RecordActivity(BoardSessionStatus),
}

impl App {
    pub(crate) async fn sync_board_state_for_thread_notification(
        &mut self,
        thread_id: ThreadId,
        notification: &ServerNotification,
    ) {
        let Some(registry) = self.board_registry.clone() else {
            return;
        };

        if let ServerNotification::ThreadNameUpdated(updated) = notification {
            let title_snapshot =
                super::boards::default_board_session_title(updated.thread_name.clone(), thread_id);
            if let Err(err) = registry
                .update_thread_board_session_title_snapshot(thread_id, &title_snapshot)
                .await
            {
                tracing::warn!(
                    error = %err,
                    thread_id = %thread_id,
                    "failed to sync app-server board session title snapshot"
                );
            }
        }

        if let Some(update) = board_status_update_for_notification(notification) {
            let result = match update {
                BoardStatusUpdate::Set(status) => {
                    registry
                        .set_thread_board_sessions_status(thread_id, status)
                        .await
                }
                BoardStatusUpdate::RecordActivity(status) => {
                    registry
                        .record_thread_board_sessions_activity(thread_id, status)
                        .await
                }
            };
            if let Err(err) = result {
                tracing::warn!(
                    error = %err,
                    thread_id = %thread_id,
                    "failed to sync app-server board session status"
                );
            }
        }

        if should_mark_thread_seen_after_notification(
            self.chat_widget.thread_id(),
            thread_id,
            notification,
        ) {
            self.mark_bound_board_session_seen(thread_id).await;
        }
    }

    pub(crate) async fn sync_board_state_for_thread_request(
        &mut self,
        thread_id: ThreadId,
        request: &ServerRequest,
    ) {
        let Some(status) = board_status_update_for_request(request) else {
            return;
        };
        let Some(registry) = self.board_registry.clone() else {
            return;
        };
        if let Err(err) = registry
            .set_thread_board_sessions_status(thread_id, status)
            .await
        {
            tracing::warn!(
                error = %err,
                thread_id = %thread_id,
                "failed to sync app-server board session request status"
            );
        }
    }

    pub(crate) async fn sync_board_state_for_outbound_op(
        &mut self,
        thread_id: ThreadId,
        op: &AppCommand,
    ) {
        let Some(status) = board_status_update_for_outbound_op(op) else {
            return;
        };
        let Some(registry) = self.board_registry.clone() else {
            return;
        };
        if let Err(err) = registry
            .set_thread_board_sessions_status(thread_id, status)
            .await
        {
            tracing::warn!(
                error = %err,
                thread_id = %thread_id,
                "failed to sync app-server board session outbound status"
            );
        }
    }

    pub(crate) async fn mark_bound_board_session_seen(&mut self, thread_id: ThreadId) {
        let Some(board_id) = self.bound_board_id.as_deref() else {
            return;
        };
        let Some(registry) = self.board_registry.clone() else {
            return;
        };
        if let Err(err) = registry.mark_board_session_seen(board_id, thread_id).await {
            tracing::warn!(
                error = %err,
                board_id,
                thread_id = %thread_id,
                "failed to mark bound app-server board session seen"
            );
        }
    }
}

fn board_status_update_for_notification(
    notification: &ServerNotification,
) -> Option<BoardStatusUpdate> {
    match notification {
        ServerNotification::TurnStarted(_) => {
            Some(BoardStatusUpdate::Set(BoardSessionStatus::Running))
        }
        ServerNotification::Error(error) if !error.will_retry => {
            Some(BoardStatusUpdate::Set(BoardSessionStatus::Errored))
        }
        ServerNotification::TurnCompleted(notification) => match notification.turn.status {
            TurnStatus::Completed | TurnStatus::Interrupted => Some(
                BoardStatusUpdate::RecordActivity(BoardSessionStatus::NeedsAttention),
            ),
            TurnStatus::Failed => Some(BoardStatusUpdate::Set(BoardSessionStatus::Errored)),
            TurnStatus::InProgress => None,
        },
        _ => None,
    }
}

fn board_status_update_for_request(request: &ServerRequest) -> Option<BoardSessionStatus> {
    match request {
        ServerRequest::CommandExecutionRequestApproval { .. }
        | ServerRequest::FileChangeRequestApproval { .. }
        | ServerRequest::McpServerElicitationRequest { .. }
        | ServerRequest::PermissionsRequestApproval { .. } => {
            Some(BoardSessionStatus::WaitingApproval)
        }
        _ => None,
    }
}

fn board_status_update_for_outbound_op(op: &AppCommand) -> Option<BoardSessionStatus> {
    match op.view() {
        AppCommandView::UserTurn { .. }
        | AppCommandView::ExecApproval { .. }
        | AppCommandView::PatchApproval { .. }
        | AppCommandView::ResolveElicitation { .. }
        | AppCommandView::UserInputAnswer { .. }
        | AppCommandView::RequestPermissionsResponse { .. } => Some(BoardSessionStatus::Running),
        _ => None,
    }
}

fn should_mark_thread_seen_after_notification(
    current_thread_id: Option<ThreadId>,
    thread_id: ThreadId,
    notification: &ServerNotification,
) -> bool {
    current_thread_id == Some(thread_id)
        && matches!(notification, ServerNotification::TurnCompleted(_))
}

#[cfg(test)]
mod tests {
    use super::BoardStatusUpdate;
    use super::board_status_update_for_notification;
    use super::board_status_update_for_outbound_op;
    use super::board_status_update_for_request;
    use super::should_mark_thread_seen_after_notification;
    use crate::app_command::AppCommand;
    use codex_app_server_protocol::ErrorNotification;
    use codex_app_server_protocol::RequestId;
    use codex_app_server_protocol::ServerNotification;
    use codex_app_server_protocol::ServerRequest;
    use codex_app_server_protocol::Turn;
    use codex_app_server_protocol::TurnStatus;
    use codex_core::boards::BoardSessionStatus;
    use codex_protocol::ThreadId;
    use codex_protocol::protocol::Op;
    use std::path::PathBuf;

    fn turn_with_status(status: TurnStatus) -> Turn {
        Turn {
            id: "turn-1".to_string(),
            items: Vec::new(),
            status,
            error: None,
        }
    }

    #[test]
    fn failed_turn_maps_to_errored() {
        assert_eq!(
            board_status_update_for_notification(&ServerNotification::TurnCompleted(
                codex_app_server_protocol::TurnCompletedNotification {
                    thread_id: "thread-1".to_string(),
                    turn: turn_with_status(TurnStatus::Failed),
                },
            )),
            Some(BoardStatusUpdate::Set(BoardSessionStatus::Errored))
        );
    }

    #[test]
    fn approval_requests_and_responses_round_trip_waiting_and_running() {
        assert_eq!(
            board_status_update_for_request(&ServerRequest::PermissionsRequestApproval {
                request_id: RequestId::String("req-1".to_string()),
                params: codex_app_server_protocol::PermissionsRequestApprovalParams {
                    thread_id: "thread-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    item_id: "perm-1".to_string(),
                    reason: Some("need access".to_string()),
                    permissions: codex_app_server_protocol::RequestPermissionProfile {
                        network: None,
                        file_system: None,
                    },
                },
            }),
            Some(BoardSessionStatus::WaitingApproval)
        );
        assert_eq!(
            board_status_update_for_outbound_op(&AppCommand::from(
                Op::RequestPermissionsResponse {
                    id: "perm-1".to_string(),
                    response: codex_protocol::request_permissions::RequestPermissionsResponse {
                        permissions:
                            codex_protocol::request_permissions::RequestPermissionProfile::default(),
                        scope: codex_protocol::request_permissions::PermissionGrantScope::Turn,
                    },
                }
            )),
            Some(BoardSessionStatus::Running)
        );
        assert_eq!(
            board_status_update_for_outbound_op(&AppCommand::from(Op::UserTurn {
                items: Vec::new(),
                cwd: PathBuf::from("/tmp"),
                approval_policy: codex_protocol::protocol::AskForApproval::OnRequest,
                sandbox_policy: codex_protocol::protocol::SandboxPolicy::new_workspace_write_policy(
                ),
                model: "gpt-5.4".to_string(),
                effort: None,
                summary: None,
                service_tier: None,
                final_output_json_schema: None,
                collaboration_mode: None,
                personality: None,
            })),
            Some(BoardSessionStatus::Running)
        );
    }

    #[test]
    fn retryable_errors_do_not_flip_status() {
        assert_eq!(
            board_status_update_for_notification(&ServerNotification::Error(ErrorNotification {
                error: codex_app_server_protocol::TurnError {
                    message: "temporary".to_string(),
                    codex_error_info: None,
                    additional_details: None,
                },
                will_retry: true,
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
            })),
            None
        );
    }

    #[test]
    fn completion_only_marks_seen_for_current_thread() {
        let current_thread_id = ThreadId::new();
        let background_thread_id = ThreadId::new();
        let completed = ServerNotification::TurnCompleted(
            codex_app_server_protocol::TurnCompletedNotification {
                thread_id: current_thread_id.to_string(),
                turn: turn_with_status(TurnStatus::Completed),
            },
        );

        assert!(should_mark_thread_seen_after_notification(
            Some(current_thread_id),
            current_thread_id,
            &completed,
        ));
        assert!(!should_mark_thread_seen_after_notification(
            Some(current_thread_id),
            background_thread_id,
            &completed,
        ));
    }
}
