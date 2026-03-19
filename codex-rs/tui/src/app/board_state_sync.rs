use super::*;
use codex_core::boards::BoardSessionStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoardStatusUpdate {
    Set(BoardSessionStatus),
    RecordActivity(BoardSessionStatus),
}

impl App {
    pub(crate) async fn sync_board_state_for_thread_event(
        &mut self,
        thread_id: ThreadId,
        event: &Event,
    ) {
        let Some(registry) = self.board_registry.clone() else {
            return;
        };

        if let EventMsg::ThreadNameUpdated(updated) = &event.msg {
            let title_snapshot =
                super::boards::default_board_session_title(updated.thread_name.clone(), thread_id);
            if let Err(err) = registry
                .update_thread_board_session_title_snapshot(thread_id, &title_snapshot)
                .await
            {
                tracing::warn!(
                    error = %err,
                    thread_id = %thread_id,
                    "failed to sync board session title snapshot"
                );
            }
        }

        if let Some(update) = board_status_update_for_event_msg(&event.msg) {
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
                    "failed to sync board session status"
                );
            }
        }

        if should_mark_thread_seen_after_event(self.chat_widget.thread_id(), thread_id, &event.msg)
        {
            self.mark_bound_board_session_seen(thread_id).await;
        }
    }

    pub(crate) async fn sync_board_state_for_outbound_op(&mut self, thread_id: ThreadId, op: &Op) {
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
                "failed to sync board session outbound status"
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
                "failed to mark bound board session seen"
            );
        }
    }
}

fn board_status_update_for_event_msg(msg: &EventMsg) -> Option<BoardStatusUpdate> {
    match msg {
        EventMsg::TurnStarted(_) => Some(BoardStatusUpdate::Set(BoardSessionStatus::Running)),
        EventMsg::ExecApprovalRequest(_)
        | EventMsg::ApplyPatchApprovalRequest(_)
        | EventMsg::ElicitationRequest(_)
        | EventMsg::RequestPermissions(_)
        | EventMsg::RequestUserInput(_) => {
            Some(BoardStatusUpdate::Set(BoardSessionStatus::WaitingApproval))
        }
        EventMsg::Error(_) | EventMsg::StreamError(_) => {
            Some(BoardStatusUpdate::Set(BoardSessionStatus::Errored))
        }
        EventMsg::TurnComplete(_) | EventMsg::TurnAborted(_) => Some(
            BoardStatusUpdate::RecordActivity(BoardSessionStatus::NeedsAttention),
        ),
        _ => None,
    }
}

fn board_status_update_for_outbound_op(op: &Op) -> Option<BoardSessionStatus> {
    match op {
        Op::UserTurn { .. }
        | Op::ExecApproval { .. }
        | Op::PatchApproval { .. }
        | Op::ResolveElicitation { .. }
        | Op::UserInputAnswer { .. }
        | Op::RequestPermissionsResponse { .. } => Some(BoardSessionStatus::Running),
        _ => None,
    }
}

fn should_mark_thread_seen_after_event(
    current_thread_id: Option<ThreadId>,
    thread_id: ThreadId,
    msg: &EventMsg,
) -> bool {
    current_thread_id == Some(thread_id)
        && matches!(msg, EventMsg::TurnComplete(_) | EventMsg::TurnAborted(_))
}

#[cfg(test)]
mod tests {
    use super::BoardStatusUpdate;
    use super::board_status_update_for_event_msg;
    use super::board_status_update_for_outbound_op;
    use super::should_mark_thread_seen_after_event;
    use codex_core::boards::BoardSessionStatus;
    use codex_protocol::ThreadId;
    use codex_protocol::protocol::EventMsg;
    use codex_protocol::protocol::Op;
    use codex_protocol::protocol::ReviewDecision;
    use std::path::PathBuf;

    #[test]
    fn completion_events_record_attention() {
        assert_eq!(
            board_status_update_for_event_msg(&EventMsg::TurnComplete(
                codex_protocol::protocol::TurnCompleteEvent {
                    turn_id: "turn-1".to_string(),
                    last_agent_message: None,
                },
            )),
            Some(BoardStatusUpdate::RecordActivity(
                BoardSessionStatus::NeedsAttention,
            ))
        );
        assert_eq!(
            board_status_update_for_event_msg(&EventMsg::TurnAborted(
                codex_protocol::protocol::TurnAbortedEvent {
                    turn_id: None,
                    reason: codex_protocol::protocol::TurnAbortReason::Interrupted,
                },
            )),
            Some(BoardStatusUpdate::RecordActivity(
                BoardSessionStatus::NeedsAttention,
            ))
        );
    }

    #[test]
    fn approval_related_events_and_ops_round_trip_waiting_and_running() {
        assert_eq!(
            board_status_update_for_event_msg(&EventMsg::RequestUserInput(
                codex_protocol::protocol::RequestUserInputEvent {
                    call_id: "call-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    questions: Vec::new(),
                },
            )),
            Some(BoardStatusUpdate::Set(BoardSessionStatus::WaitingApproval))
        );
        assert_eq!(
            board_status_update_for_outbound_op(&Op::UserInputAnswer {
                id: "call-1".to_string(),
                response: codex_protocol::request_user_input::RequestUserInputResponse {
                    answers: Default::default(),
                },
            }),
            Some(BoardSessionStatus::Running)
        );
        assert_eq!(
            board_status_update_for_outbound_op(&Op::ExecApproval {
                id: "approval-1".to_string(),
                turn_id: None,
                decision: ReviewDecision::Approved,
            }),
            Some(BoardSessionStatus::Running)
        );
        assert_eq!(
            board_status_update_for_outbound_op(&Op::UserTurn {
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
            }),
            Some(BoardSessionStatus::Running)
        );
    }

    #[test]
    fn completion_only_marks_seen_for_current_thread() {
        let current_thread_id = ThreadId::new();
        let background_thread_id = ThreadId::new();
        let completed = EventMsg::TurnComplete(codex_protocol::protocol::TurnCompleteEvent {
            turn_id: "turn-1".to_string(),
            last_agent_message: None,
        });

        assert!(should_mark_thread_seen_after_event(
            Some(current_thread_id),
            current_thread_id,
            &completed,
        ));
        assert!(!should_mark_thread_seen_after_event(
            Some(current_thread_id),
            background_thread_id,
            &completed,
        ));
    }
}
