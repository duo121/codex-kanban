use super::*;
use crate::app_server_session::AppServerStartedThread;
use crate::bottom_pane::SelectionHotkey;
use crate::bottom_pane::SelectionHotkeyScope;
use crate::bottom_pane::custom_prompt_view::CustomPromptView;
use codex_core::boards::BoardOverview;
use codex_core::boards::BoardRegistry;
use codex_core::boards::BoardSession;
use codex_core::boards::BoardSessionMoveDirection;
use codex_core::boards::BoardSessionStatus;

const BOARD_PICKER_VIEW_ID: &str = "kb-board-picker";
const BOARD_SESSION_PICKER_VIEW_ID: &str = "kb-board-session-picker";

impl App {
    pub(crate) fn board_registry(&mut self) -> Option<BoardRegistry> {
        let registry = self.board_registry.clone();
        if registry.is_none() {
            self.chat_widget
                .add_error_message("Board persistence is unavailable in this session.".to_string());
        }
        registry
    }

    pub(crate) async fn open_board_picker(&mut self) {
        let Some(registry) = self.board_registry() else {
            return;
        };
        match registry.list_boards().await {
            Ok(boards) => self.show_board_picker(boards),
            Err(err) => self
                .chat_widget
                .add_error_message(format!("Failed to load boards: {err}")),
        }
    }

    pub(crate) async fn open_current_board_session_picker(
        &mut self,
        app_server: &mut AppServerSession,
    ) {
        let Some(board_id) = self.bound_board_id.clone() else {
            self.chat_widget.add_info_message(
                "This window is not bound to a board.".to_string(),
                Some("Use /kb to choose or create a board.".to_string()),
            );
            return;
        };
        self.open_board_session_picker_for_board(
            app_server, board_id, /*preferred_thread_id*/ None,
        )
        .await;
    }

    async fn open_board_session_picker_for_board(
        &mut self,
        app_server: &mut AppServerSession,
        board_id: String,
        preferred_thread_id: Option<ThreadId>,
    ) {
        let Some(registry) = self.board_registry() else {
            return;
        };
        let sessions = match registry.list_board_sessions(&board_id).await {
            Ok(sessions) => sessions,
            Err(err) => {
                self.chat_widget
                    .add_error_message(format!("Failed to load board sessions: {err}"));
                return;
            }
        };
        if sessions.is_empty() {
            if let Err(err) = self
                .create_bound_board_session_in_board(
                    /*tui*/ None,
                    app_server,
                    board_id.clone(),
                )
                .await
            {
                self.chat_widget
                    .add_error_message(format!("Failed to create a board session: {err}"));
                return;
            }
            match registry.list_board_sessions(&board_id).await {
                Ok(sessions) if !sessions.is_empty() => {
                    self.show_board_session_picker(board_id, sessions, preferred_thread_id);
                }
                Ok(_) => {}
                Err(err) => self
                    .chat_widget
                    .add_error_message(format!("Failed to load board sessions: {err}")),
            }
            return;
        }
        self.show_board_session_picker(board_id, sessions, preferred_thread_id);
    }

    pub(crate) fn open_create_board_prompt(&mut self) {
        let tx = self.app_event_tx.clone();
        let view = CustomPromptView::new(
            "New board".to_string(),
            "Type a board name and press Enter".to_string(),
            /*context_label*/ None,
            Box::new(move |name: String| {
                let name = name.trim().to_string();
                if name.is_empty() {
                    tx.send(AppEvent::InsertHistoryCell(Box::new(
                        history_cell::new_error_event("Board name cannot be empty.".to_string()),
                    )));
                    return;
                }
                tx.send(AppEvent::CreateBoard { name });
            }),
        );
        self.chat_widget.show_view(Box::new(view));
    }

    pub(crate) fn open_rename_board_prompt(&mut self, board_id: String, current_name: String) {
        let tx = self.app_event_tx.clone();
        let view = CustomPromptView::new(
            "Rename board".to_string(),
            "Type a new board name and press Enter".to_string(),
            Some(format!("Current: {current_name}")),
            Box::new(move |name: String| {
                let name = name.trim().to_string();
                if name.is_empty() {
                    tx.send(AppEvent::InsertHistoryCell(Box::new(
                        history_cell::new_error_event("Board name cannot be empty.".to_string()),
                    )));
                    return;
                }
                tx.send(AppEvent::RenameBoard {
                    board_id: board_id.clone(),
                    name,
                });
            }),
        );
        self.chat_widget.show_view(Box::new(view));
    }

    pub(crate) fn open_delete_board_prompt(&mut self, board_id: String, board_name: String) {
        self.chat_widget.show_selection_view(SelectionViewParams {
            title: Some("Delete board?".to_string()),
            subtitle: Some(board_name),
            footer_hint: Some(confirm_picker_hint_line("delete", "cancel")),
            items: vec![
                SelectionItem {
                    name: "Delete board".to_string(),
                    description: Some(
                        "Board metadata and board-session mappings will be removed.".to_string(),
                    ),
                    actions: vec![Box::new(move |tx| {
                        tx.send(AppEvent::DeleteBoard {
                            board_id: board_id.clone(),
                        });
                    })],
                    dismiss_on_select: true,
                    ..Default::default()
                },
                SelectionItem {
                    name: "Cancel".to_string(),
                    dismiss_on_select: true,
                    ..Default::default()
                },
            ],
            ..Default::default()
        });
    }

    pub(crate) fn open_remove_board_session_prompt(
        &mut self,
        board_id: String,
        thread_id: ThreadId,
        title: String,
    ) {
        self.chat_widget.show_selection_view(SelectionViewParams {
            title: Some("Remove session from board?".to_string()),
            subtitle: Some(title),
            footer_hint: Some(confirm_picker_hint_line("remove", "cancel")),
            items: vec![
                SelectionItem {
                    name: "Remove from board".to_string(),
                    description: Some(
                        "The underlying chat history stays available via /resume.".to_string(),
                    ),
                    actions: vec![Box::new(move |tx| {
                        tx.send(AppEvent::RemoveBoardSession {
                            board_id: board_id.clone(),
                            thread_id,
                        });
                    })],
                    dismiss_on_select: true,
                    ..Default::default()
                },
                SelectionItem {
                    name: "Cancel".to_string(),
                    dismiss_on_select: true,
                    ..Default::default()
                },
            ],
            ..Default::default()
        });
    }

    pub(crate) fn open_rename_board_session_prompt(
        &mut self,
        board_id: String,
        thread_id: ThreadId,
        current_name: String,
    ) {
        let tx = self.app_event_tx.clone();
        let view = CustomPromptView::new(
            "Rename board session".to_string(),
            "Type a new session name and press Enter".to_string(),
            Some(format!("Current: {current_name}")),
            Box::new(move |name: String| {
                let Some(name) = codex_core::util::normalize_thread_name(&name) else {
                    tx.send(AppEvent::InsertHistoryCell(Box::new(
                        history_cell::new_error_event("Thread name cannot be empty.".to_string()),
                    )));
                    return;
                };
                tx.send(AppEvent::RenameBoardSession {
                    board_id: board_id.clone(),
                    thread_id,
                    name,
                });
            }),
        );
        self.chat_widget.show_view(Box::new(view));
    }

    pub(crate) async fn create_board(&mut self, name: String) {
        let Some(registry) = self.board_registry() else {
            return;
        };
        match registry.create_board(&name).await {
            Ok(_) => self.open_board_picker().await,
            Err(err) => self
                .chat_widget
                .add_error_message(format!("Failed to create board: {err}")),
        }
    }

    pub(crate) async fn rename_board(&mut self, board_id: String, name: String) {
        let Some(registry) = self.board_registry() else {
            return;
        };
        match registry.rename_board(&board_id, &name).await {
            Ok(true) => self.open_board_picker().await,
            Ok(false) => self
                .chat_widget
                .add_error_message("Board no longer exists.".to_string()),
            Err(err) => self
                .chat_widget
                .add_error_message(format!("Failed to rename board: {err}")),
        }
    }

    pub(crate) async fn delete_board(&mut self, board_id: String) {
        let Some(registry) = self.board_registry() else {
            return;
        };
        match registry.delete_board(&board_id).await {
            Ok(true) => {
                if self.bound_board_id.as_deref() == Some(board_id.as_str()) {
                    self.bound_board_id = None;
                    self.chat_widget.add_info_message(
                        "Board deleted. This window is now unbound.".to_string(),
                        Some("Use /kb to choose another board.".to_string()),
                    );
                }
                self.open_board_picker().await;
            }
            Ok(false) => self
                .chat_widget
                .add_error_message("Board no longer exists.".to_string()),
            Err(err) => self
                .chat_widget
                .add_error_message(format!("Failed to delete board: {err}")),
        }
    }

    pub(crate) async fn bind_board(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        board_id: String,
        open_sessions_after: bool,
    ) -> Result<()> {
        let Some(registry) = self.board_registry() else {
            return Ok(());
        };
        let Some(board) = registry
            .get_board(&board_id)
            .await
            .map_err(board_registry_report)?
        else {
            self.bound_board_id = None;
            self.chat_widget
                .add_error_message("Board no longer exists.".to_string());
            return Ok(());
        };

        self.bound_board_id = Some(board_id.clone());

        let sessions = registry
            .list_board_sessions(&board_id)
            .await
            .map_err(board_registry_report)?;
        if sessions.is_empty() {
            self.create_bound_board_session_in_board(Some(tui), app_server, board_id.clone())
                .await?;
        } else {
            let target_thread_id = board
                .last_selected_thread_id
                .filter(|thread_id| {
                    sessions
                        .iter()
                        .any(|session| session.thread_id == *thread_id)
                })
                .unwrap_or(sessions[0].thread_id);
            if !self
                .switch_to_board_session(tui, app_server, board_id.clone(), target_thread_id)
                .await?
            {
                self.create_bound_board_session_in_board(Some(tui), app_server, board_id.clone())
                    .await?;
            }
        }

        if open_sessions_after {
            self.open_current_board_session_picker(app_server).await;
        }
        Ok(())
    }

    pub(crate) async fn create_bound_board_session(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
    ) -> Result<()> {
        let Some(board_id) = self.bound_board_id.clone() else {
            self.chat_widget.add_info_message(
                "This window is not bound to a board.".to_string(),
                Some("Use /kb to choose or create a board.".to_string()),
            );
            return Ok(());
        };
        self.create_bound_board_session_in_board(Some(tui), app_server, board_id)
            .await?;
        self.open_current_board_session_picker(app_server).await;
        Ok(())
    }

    pub(crate) async fn switch_to_board_session(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        board_id: String,
        thread_id: ThreadId,
    ) -> Result<bool> {
        self.bound_board_id = Some(board_id.clone());
        let Some(available_thread_id) = self
            .ensure_board_thread_available(app_server, board_id.as_str(), thread_id)
            .await?
        else {
            return Ok(false);
        };
        self.select_agent_thread(tui, app_server, available_thread_id)
            .await?;
        self.set_board_last_selected_thread(&board_id, available_thread_id)
            .await;
        Ok(true)
    }

    pub(crate) async fn remove_board_session(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        board_id: String,
        thread_id: ThreadId,
    ) -> Result<()> {
        let Some(registry) = self.board_registry() else {
            return Ok(());
        };
        let removed_session_idx = if self.chat_widget.thread_id() == Some(thread_id) {
            registry
                .list_board_sessions(&board_id)
                .await
                .map_err(board_registry_report)?
                .iter()
                .position(|session| session.thread_id == thread_id)
        } else {
            None
        };
        match registry.remove_board_session(&board_id, thread_id).await {
            Ok(true) => {}
            Ok(false) => {
                self.chat_widget
                    .add_error_message("Board session no longer exists.".to_string());
                return Ok(());
            }
            Err(err) => {
                self.chat_widget
                    .add_error_message(format!("Failed to remove board session: {err}"));
                return Ok(());
            }
        }

        let is_current = self.chat_widget.thread_id() == Some(thread_id);
        if !is_current {
            self.open_current_board_session_picker(app_server).await;
            return Ok(());
        }

        let remaining_sessions = registry
            .list_board_sessions(&board_id)
            .await
            .map_err(board_registry_report)?;
        if let Some(next_thread_id) =
            replacement_thread_id_after_removal(removed_session_idx, &remaining_sessions)
        {
            let _ = self
                .switch_to_board_session(tui, app_server, board_id.clone(), next_thread_id)
                .await?;
        } else {
            self.create_bound_board_session_in_board(Some(tui), app_server, board_id.clone())
                .await?;
        }
        self.open_current_board_session_picker(app_server).await;
        Ok(())
    }

    pub(crate) async fn rename_board_session(
        &mut self,
        app_server: &mut AppServerSession,
        board_id: String,
        thread_id: ThreadId,
        name: String,
    ) -> Result<()> {
        let Some(registry) = self.board_registry() else {
            return Ok(());
        };
        let Some(thread_id) = self
            .ensure_board_thread_available(app_server, board_id.as_str(), thread_id)
            .await?
        else {
            return Ok(());
        };

        let op = AppCommand::set_thread_name(name.clone());
        if let Err(err) = self.submit_thread_op(app_server, thread_id, op).await {
            self.chat_widget
                .add_error_message(format!("Failed to rename board session: {err}"));
        } else if let Err(err) = registry
            .update_thread_board_session_title_snapshot(thread_id, &name)
            .await
        {
            self.chat_widget
                .add_error_message(format!("Failed to update board session title: {err}"));
        }

        self.open_board_session_picker_for_board(app_server, board_id, Some(thread_id))
            .await;
        Ok(())
    }

    pub(crate) async fn move_board_session(
        &mut self,
        app_server: &mut AppServerSession,
        board_id: String,
        thread_id: ThreadId,
        direction: BoardSessionMoveDirection,
    ) -> Result<()> {
        let Some(registry) = self.board_registry() else {
            return Ok(());
        };

        match registry
            .move_board_session(&board_id, thread_id, direction)
            .await
        {
            Ok(true) => {}
            Ok(false) => {
                self.chat_widget.add_info_message(
                    format!(
                        "Session is already at the {} of the board.",
                        board_session_move_edge_label(direction)
                    ),
                    /*hint*/ None,
                );
            }
            Err(err) => {
                self.chat_widget
                    .add_error_message(format!("Failed to move board session: {err}"));
            }
        }

        self.open_board_session_picker_for_board(app_server, board_id, Some(thread_id))
            .await;
        Ok(())
    }

    async fn create_bound_board_session_in_board(
        &mut self,
        tui: Option<&mut tui::Tui>,
        app_server: &mut AppServerSession,
        board_id: String,
    ) -> Result<()> {
        let Some(registry) = self.board_registry() else {
            return Ok(());
        };

        self.refresh_in_memory_config_from_disk_best_effort("starting a board session")
            .await;

        match app_server.start_thread(&self.config).await {
            Ok(started) => {
                let title_snapshot = default_board_session_title(
                    started.session.thread_name.clone(),
                    started.session.thread_id,
                );
                let thread_id = self.register_app_server_thread(started).await;
                registry
                    .add_board_session(&board_id, thread_id, &title_snapshot)
                    .await
                    .map_err(board_registry_report)?;
                self.set_board_last_selected_thread(&board_id, thread_id)
                    .await;
                if let Some(tui) = tui {
                    let _ = self
                        .switch_to_board_session(tui, app_server, board_id.clone(), thread_id)
                        .await?;
                }
            }
            Err(err) => self
                .chat_widget
                .add_error_message(format!("Failed to start a board session: {err}")),
        }
        Ok(())
    }

    async fn recover_missing_board_session(
        &mut self,
        app_server: &mut AppServerSession,
        board_id: &str,
        missing_thread_id: ThreadId,
    ) -> Result<Option<ThreadId>> {
        let Some(registry) = self.board_registry.clone() else {
            return Ok(None);
        };
        let Some(missing_session) = registry
            .list_board_sessions(board_id)
            .await
            .map_err(board_registry_report)?
            .into_iter()
            .find(|session| session.thread_id == missing_thread_id)
        else {
            return Ok(None);
        };

        self.refresh_in_memory_config_from_disk_best_effort("recovering a board session")
            .await;

        match app_server.start_thread(&self.config).await {
            Ok(started) => {
                let title_snapshot = recovered_board_session_title(
                    missing_session.title_snapshot.as_str(),
                    missing_thread_id,
                    started.session.thread_name.clone(),
                    started.session.thread_id,
                );
                let new_thread_id = self.register_app_server_thread(started).await;
                let replaced = registry
                    .replace_board_session_thread_id(
                        board_id,
                        missing_thread_id,
                        new_thread_id,
                        &title_snapshot,
                    )
                    .await
                    .map_err(board_registry_report)?;
                if !replaced {
                    return Ok(None);
                }
                self.chat_widget.add_info_message(
                    "Recovered an empty board session that had no saved transcript yet."
                        .to_string(),
                    /*hint*/ None,
                );
                Ok(Some(new_thread_id))
            }
            Err(err) => {
                self.chat_widget.add_error_message(format!(
                    "Failed to recover board session {missing_thread_id}: {err}",
                ));
                Ok(None)
            }
        }
    }

    async fn ensure_board_thread_available(
        &mut self,
        app_server: &mut AppServerSession,
        board_id: &str,
        thread_id: ThreadId,
    ) -> Result<Option<ThreadId>> {
        if self.thread_event_channels.contains_key(&thread_id) {
            return Ok(Some(thread_id));
        }

        let Some(registry) = self.board_registry.clone() else {
            return Ok(None);
        };
        if registry.find_rollout_path_by_id(thread_id).await.is_none() {
            return self
                .recover_missing_board_session(app_server, board_id, thread_id)
                .await;
        }

        match app_server
            .resume_thread(self.config.clone(), thread_id)
            .await
        {
            Ok(started) => {
                if started.session.thread_id != thread_id {
                    self.chat_widget.add_error_message(format!(
                        "Resumed board session id mismatch: expected {thread_id}, got {}.",
                        started.session.thread_id,
                    ));
                    return Ok(None);
                }
                self.register_app_server_thread(started).await;
                Ok(Some(thread_id))
            }
            Err(err) => {
                self.chat_widget.add_error_message(format!(
                    "Failed to resume board session {thread_id}: {err}",
                ));
                Ok(None)
            }
        }
    }

    async fn register_app_server_thread(&mut self, started: AppServerStartedThread) -> ThreadId {
        let AppServerStartedThread { session, turns } = started;
        let thread_id = session.thread_id;
        self.upsert_agent_picker_thread(
            thread_id, /*agent_nickname*/ None, /*agent_role*/ None,
            /*is_closed*/ false,
        );
        if let Some(channel) = self.thread_event_channels.get(&thread_id) {
            let mut store = channel.store.lock().await;
            store.set_session(session, turns);
        } else {
            self.thread_event_channels.insert(
                thread_id,
                ThreadEventChannel::new_with_session(THREAD_EVENT_CHANNEL_CAPACITY, session, turns),
            );
        }
        thread_id
    }

    async fn set_board_last_selected_thread(&mut self, board_id: &str, thread_id: ThreadId) {
        let Some(registry) = self.board_registry.clone() else {
            return;
        };
        if let Err(err) = registry
            .set_board_last_selected_thread(board_id, Some(thread_id))
            .await
        {
            tracing::warn!(error = %err, board_id, "failed to persist last selected board thread");
        }
    }

    fn show_board_picker(&mut self, boards: Vec<BoardOverview>) {
        let boards = Arc::new(boards);
        let initial_selected_idx = self.bound_board_id.as_ref().and_then(|bound_board_id| {
            boards
                .iter()
                .position(|board| board.board.id.as_str() == bound_board_id.as_str())
        });

        let items = boards
            .iter()
            .map(|board| SelectionItem {
                name: board.board.name.clone(),
                description: Some(board_overview_description(board)),
                is_current: self
                    .bound_board_id
                    .as_deref()
                    .is_some_and(|bound_board_id| bound_board_id == board.board.id.as_str()),
                actions: vec![{
                    let board_id = board.board.id.clone();
                    Box::new(move |tx| {
                        tx.send(AppEvent::BindBoard {
                            board_id: board_id.clone(),
                            open_sessions_after: true,
                        });
                    })
                }],
                dismiss_on_select: true,
                search_value: Some(format!("{} {}", board.board.name, board.board.id)),
                ..Default::default()
            })
            .collect();

        self.chat_widget.show_selection_view(SelectionViewParams {
            view_id: Some(BOARD_PICKER_VIEW_ID),
            title: Some("Boards".to_string()),
            subtitle: Some("Bind this window to a board".to_string()),
            footer_hint: Some(board_picker_hint_line()),
            items,
            is_searchable: true,
            search_placeholder: Some("Type to search boards".to_string()),
            initial_selected_idx,
            hotkeys: vec![
                SelectionHotkey {
                    binding: crate::key_hint::plain(KeyCode::Char('n')),
                    action: Box::new(|_selected_idx, tx| {
                        tx.send(AppEvent::OpenCreateBoardPrompt);
                    }),
                    dismiss_view: true,
                    scope: SelectionHotkeyScope::WhenSearchEmpty,
                },
                SelectionHotkey {
                    binding: crate::key_hint::plain(KeyCode::Char('r')),
                    action: {
                        let boards = Arc::clone(&boards);
                        Box::new(move |selected_idx, tx| {
                            let Some(board) = selected_idx.and_then(|idx| boards.get(idx)) else {
                                return;
                            };
                            tx.send(AppEvent::OpenRenameBoardPrompt {
                                board_id: board.board.id.clone(),
                                current_name: board.board.name.clone(),
                            });
                        })
                    },
                    dismiss_view: true,
                    scope: SelectionHotkeyScope::WhenSearchEmpty,
                },
                SelectionHotkey {
                    binding: crate::key_hint::plain(KeyCode::Char('d')),
                    action: {
                        let boards = Arc::clone(&boards);
                        Box::new(move |selected_idx, tx| {
                            let Some(board) = selected_idx.and_then(|idx| boards.get(idx)) else {
                                return;
                            };
                            tx.send(AppEvent::OpenDeleteBoardPrompt {
                                board_id: board.board.id.clone(),
                                board_name: board.board.name.clone(),
                            });
                        })
                    },
                    dismiss_view: true,
                    scope: SelectionHotkeyScope::WhenSearchEmpty,
                },
            ],
            ..Default::default()
        });
    }

    fn show_board_session_picker(
        &mut self,
        board_id: String,
        sessions: Vec<BoardSession>,
        preferred_thread_id: Option<ThreadId>,
    ) {
        let sessions = Arc::new(sessions);
        let initial_selected_idx = board_session_picker_selected_idx(
            sessions.as_ref(),
            preferred_thread_id,
            self.chat_widget.thread_id(),
        );

        let items = sessions
            .iter()
            .map(|session| SelectionItem {
                name: board_session_title(session),
                description: Some(board_session_description(session)),
                is_current: self.chat_widget.thread_id() == Some(session.thread_id),
                actions: vec![{
                    let board_id = board_id.clone();
                    let thread_id = session.thread_id;
                    Box::new(move |tx| {
                        tx.send(AppEvent::SwitchBoardSession {
                            board_id: board_id.clone(),
                            thread_id,
                        });
                    })
                }],
                dismiss_on_select: true,
                search_value: Some(format!(
                    "{} {}",
                    board_session_title(session),
                    session.thread_id,
                )),
                ..Default::default()
            })
            .collect();

        self.chat_widget.show_selection_view(SelectionViewParams {
            view_id: Some(BOARD_SESSION_PICKER_VIEW_ID),
            title: Some("Board Sessions".to_string()),
            subtitle: Some("Switch sessions in the current board".to_string()),
            footer_hint: Some(board_sessions_hint_line()),
            items,
            is_searchable: true,
            search_placeholder: Some("Type to search sessions".to_string()),
            initial_selected_idx,
            hotkeys: vec![
                SelectionHotkey {
                    binding: crate::key_hint::plain(KeyCode::Char('n')),
                    action: Box::new(|_selected_idx, tx| {
                        tx.send(AppEvent::CreateBoundBoardSession);
                    }),
                    dismiss_view: true,
                    scope: SelectionHotkeyScope::WhenSearchEmpty,
                },
                SelectionHotkey {
                    binding: crate::key_hint::plain(KeyCode::Char('d')),
                    action: {
                        let sessions = Arc::clone(&sessions);
                        let board_id = board_id.clone();
                        Box::new(move |selected_idx, tx| {
                            let Some(session) = selected_idx.and_then(|idx| sessions.get(idx))
                            else {
                                return;
                            };
                            tx.send(AppEvent::OpenRemoveBoardSessionPrompt {
                                board_id: board_id.clone(),
                                thread_id: session.thread_id,
                                title: board_session_title(session),
                            });
                        })
                    },
                    dismiss_view: true,
                    scope: SelectionHotkeyScope::WhenSearchEmpty,
                },
                SelectionHotkey {
                    binding: crate::key_hint::plain(KeyCode::Char('r')),
                    action: {
                        let sessions = Arc::clone(&sessions);
                        let board_id = board_id.clone();
                        Box::new(move |selected_idx, tx| {
                            let Some(session) = selected_idx.and_then(|idx| sessions.get(idx))
                            else {
                                return;
                            };
                            tx.send(AppEvent::OpenRenameBoardSessionPrompt {
                                board_id: board_id.clone(),
                                thread_id: session.thread_id,
                                current_name: board_session_title(session),
                            });
                        })
                    },
                    dismiss_view: true,
                    scope: SelectionHotkeyScope::WhenSearchEmpty,
                },
                SelectionHotkey {
                    binding: crate::key_hint::shift(KeyCode::Up),
                    action: {
                        let sessions = Arc::clone(&sessions);
                        let board_id = board_id.clone();
                        Box::new(move |selected_idx, tx| {
                            let Some(session) = selected_idx.and_then(|idx| sessions.get(idx))
                            else {
                                return;
                            };
                            tx.send(AppEvent::MoveBoardSession {
                                board_id: board_id.clone(),
                                thread_id: session.thread_id,
                                direction: BoardSessionMoveDirection::Up,
                            });
                        })
                    },
                    dismiss_view: true,
                    scope: SelectionHotkeyScope::WhenSearchEmpty,
                },
                SelectionHotkey {
                    binding: crate::key_hint::shift(KeyCode::Down),
                    action: {
                        let sessions = Arc::clone(&sessions);
                        let move_board_id = board_id;
                        Box::new(move |selected_idx, tx| {
                            let Some(session) = selected_idx.and_then(|idx| sessions.get(idx))
                            else {
                                return;
                            };
                            tx.send(AppEvent::MoveBoardSession {
                                board_id: move_board_id.clone(),
                                thread_id: session.thread_id,
                                direction: BoardSessionMoveDirection::Down,
                            });
                        })
                    },
                    dismiss_view: true,
                    scope: SelectionHotkeyScope::WhenSearchEmpty,
                },
            ],
            ..Default::default()
        });
    }
}

fn board_overview_description(board: &BoardOverview) -> String {
    format!(
        "{} sessions · {} running · {} needs attention",
        board.session_count, board.running_count, board.needs_attention_count,
    )
}

fn board_session_title(session: &BoardSession) -> String {
    if session.title_snapshot.trim().is_empty() {
        default_board_session_title(/*thread_name*/ None, session.thread_id)
    } else {
        session.title_snapshot.clone()
    }
}

fn board_session_description(session: &BoardSession) -> String {
    board_session_status_label(session.status).to_string()
}

fn board_session_status_label(status: BoardSessionStatus) -> &'static str {
    match status {
        BoardSessionStatus::Unknown => "unknown",
        BoardSessionStatus::Running => "running",
        BoardSessionStatus::NeedsAttention => "needs attention",
        BoardSessionStatus::Seen => "seen",
        BoardSessionStatus::WaitingApproval => "waiting approval",
        BoardSessionStatus::Errored => "errored",
    }
}

pub(crate) fn default_board_session_title(
    thread_name: Option<String>,
    thread_id: ThreadId,
) -> String {
    if let Some(thread_name) = thread_name.filter(|name| !name.trim().is_empty()) {
        return thread_name;
    }
    let thread_id = thread_id.to_string();
    let short_id = thread_id.get(..8).unwrap_or(thread_id.as_str());
    format!("Untitled {short_id}")
}

fn recovered_board_session_title(
    current_title: &str,
    old_thread_id: ThreadId,
    new_thread_name: Option<String>,
    new_thread_id: ThreadId,
) -> String {
    let old_default_title = default_board_session_title(/*thread_name*/ None, old_thread_id);
    if current_title.trim().is_empty() || current_title == old_default_title {
        return default_board_session_title(new_thread_name, new_thread_id);
    }
    current_title.to_string()
}

fn board_picker_hint_line() -> Line<'static> {
    vec![
        crate::key_hint::plain(KeyCode::Enter).into(),
        " bind".dim(),
        "  ".into(),
        crate::key_hint::plain(KeyCode::Char('n')).into(),
        " create".dim(),
        "  ".into(),
        crate::key_hint::plain(KeyCode::Char('r')).into(),
        " rename".dim(),
        "  ".into(),
        crate::key_hint::plain(KeyCode::Char('d')).into(),
        " delete".dim(),
        "  esc close".dim(),
    ]
    .into()
}

fn board_sessions_hint_line() -> Line<'static> {
    vec![
        crate::key_hint::plain(KeyCode::Enter).into(),
        " switch".dim(),
        "  ".into(),
        crate::key_hint::plain(KeyCode::Char('n')).into(),
        " new".dim(),
        "  ".into(),
        crate::key_hint::plain(KeyCode::Char('r')).into(),
        " rename".dim(),
        "  ".into(),
        crate::key_hint::plain(KeyCode::Char('d')).into(),
        " remove".dim(),
        "  ".into(),
        crate::key_hint::shift(KeyCode::Up).into(),
        "/".dim(),
        crate::key_hint::shift(KeyCode::Down).into(),
        " move".dim(),
        "  esc close".dim(),
    ]
    .into()
}

fn board_session_picker_selected_idx(
    sessions: &[BoardSession],
    preferred_thread_id: Option<ThreadId>,
    current_thread_id: Option<ThreadId>,
) -> Option<usize> {
    preferred_thread_id
        .or(current_thread_id)
        .and_then(|thread_id| {
            sessions
                .iter()
                .position(|session| session.thread_id == thread_id)
        })
}

fn replacement_thread_id_after_removal(
    removed_session_idx: Option<usize>,
    remaining_sessions: &[BoardSession],
) -> Option<ThreadId> {
    let replacement_idx = removed_session_idx
        .unwrap_or(0)
        .min(remaining_sessions.len().checked_sub(1)?);
    remaining_sessions
        .get(replacement_idx)
        .map(|session| session.thread_id)
}

fn board_session_move_edge_label(direction: BoardSessionMoveDirection) -> &'static str {
    match direction {
        BoardSessionMoveDirection::Up => "top",
        BoardSessionMoveDirection::Down => "bottom",
    }
}

fn confirm_picker_hint_line(confirm_action: &str, cancel_action: &str) -> Line<'static> {
    Line::from(format!("enter {confirm_action}  esc {cancel_action}").dim())
}

fn board_registry_report(err: anyhow::Error) -> color_eyre::Report {
    color_eyre::eyre::eyre!("{err}")
}
