//! Input handling — translates crossterm key/mouse events into state mutations.

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};

use crate::input::TerminalKey;
use ratatui::layout::Direction;

/// What the cursor is over in the ACTIONS sidebar panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ActionPanelHit {
    /// The header "new" button.
    New,
    /// The play button of the action with this id.
    Play(u64),
    /// The (non-play) body of the action row with this id — opens the editor.
    Edit(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScrollbarClickTarget {
    Thumb { grab_row_offset: u16 },
    Track { offset_from_bottom: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(test)]
enum WheelRouting {
    HostScroll,
    MouseReport,
    AlternateScroll,
}

const WORKSPACE_DRAG_THRESHOLD: u16 = 1;
const TAB_DRAG_THRESHOLD: u16 = 1;

mod modal;
mod mouse;
mod navigate;
mod overlays;
mod selection;
mod settings;
mod sidebar;
mod terminal;

pub(crate) use self::{
    modal::{
        handle_action_editor_key, handle_actions_menu_key, handle_confirm_close_key,
        handle_context_menu_key, handle_global_menu_key, handle_keybind_help_key,
        handle_rename_key, handle_resize_key,
    },
    navigate::terminal_direct_navigation_action,
    settings::open_settings,
};
use self::{
    modal::{
        modal_action_from_key, ModalAction, ONBOARDING_WELCOME_ACTIONS, RELEASE_NOTES_ACTIONS,
    },
    settings::SettingsAction,
};
use super::state::{AppState, Mode};
use super::App;

// ---------------------------------------------------------------------------
// Key handling
// ---------------------------------------------------------------------------

impl App {
    pub(super) async fn handle_key(&mut self, key: TerminalKey) {
        match self.state.mode {
            Mode::Terminal => self.handle_terminal_key(key).await,
            Mode::Navigate => self.handle_navigate_key(key),
            _ => {
                let key = key.as_key_event();
                match self.state.mode {
                    Mode::Onboarding => self.handle_onboarding_key(key),
                    Mode::ReleaseNotes => self.handle_release_notes_key(key),
                    Mode::Navigate => unreachable!(),
                    Mode::RenameWorkspace | Mode::RenameTab | Mode::RenamePane => {
                        handle_rename_key(&mut self.state, key)
                    }
                    Mode::Resize => handle_resize_key(&mut self.state, key),
                    Mode::ConfirmClose => handle_confirm_close_key(&mut self.state, key),
                    Mode::ContextMenu => handle_context_menu_key(&mut self.state, key),
                    Mode::Settings => self.handle_settings_key(key),
                    Mode::GlobalMenu => handle_global_menu_key(&mut self.state, key),
                    Mode::ActionsMenu => handle_actions_menu_key(&mut self.state, key),
                    Mode::KeybindHelp => handle_keybind_help_key(&mut self.state, key),
                    Mode::ActionEditor => handle_action_editor_key(&mut self.state, key),
                    Mode::Terminal => unreachable!(),
                }
            }
        }
    }

    pub(super) async fn handle_paste(&mut self, text: String) {
        if self.state.mode != Mode::Terminal {
            return;
        }
        if let Some(ws_idx) = self.state.active {
            if let Some(rt) = self.state.focused_runtime_in_workspace(ws_idx) {
                let _ = rt.send_paste(text).await;
            }
        }
    }

    pub(crate) fn handle_onboarding_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Right | KeyCode::Char('l') => self.open_settings_from_onboarding(),
            _ => {
                if let Some(ModalAction::Continue) =
                    modal_action_from_key(&key, ONBOARDING_WELCOME_ACTIONS)
                {
                    self.open_settings_from_onboarding();
                }
            }
        }
    }

    pub(crate) fn handle_release_notes_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.scroll_release_notes(-1),
            KeyCode::Down | KeyCode::Char('j') => self.scroll_release_notes(1),
            KeyCode::PageUp => self.scroll_release_notes(-8),
            KeyCode::PageDown => self.scroll_release_notes(8),
            KeyCode::Home => {
                if let Some(notes) = &mut self.state.release_notes {
                    notes.scroll = 0;
                }
            }
            KeyCode::End => {
                let max_scroll = self.state.release_notes_max_scroll();
                if let Some(notes) = &mut self.state.release_notes {
                    notes.scroll = max_scroll;
                }
            }
            _ => {
                if let Some(ModalAction::Close) = modal_action_from_key(&key, RELEASE_NOTES_ACTIONS)
                {
                    self.dismiss_release_notes();
                }
            }
        }
    }

    pub(super) fn handle_mouse(&mut self, mouse: MouseEvent) {
        if self.handle_overlay_mouse(mouse) {
            return;
        }

        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
            && self.state.on_sidebar_divider(mouse.column, mouse.row)
        {
            let now = std::time::Instant::now();
            let is_double_click = self
                .last_sidebar_divider_click
                .is_some_and(|last| now.duration_since(last) <= super::SIDEBAR_DOUBLE_CLICK_WINDOW);
            self.last_sidebar_divider_click = Some(now);

            if is_double_click {
                self.state.sidebar_width = self.state.default_sidebar_width;
                self.state.sidebar_width_source =
                    crate::app::state::SidebarWidthSource::ConfigDefault;
                self.state.sidebar_width_auto = false;
                self.state.mark_session_dirty();
                self.state.drag = None;
                return;
            }
        }

        let previous_agent_panel_scope = self.state.agent_panel_scope;
        if let Some(action) = self.state.handle_mouse(mouse) {
            match action {
                SettingsAction::SaveTheme(name) => self.save_theme(&name),
                SettingsAction::SaveSound(enabled) => self.save_sound(enabled),
                SettingsAction::SaveToastDelivery(delivery) => self.save_toast_delivery(delivery),
                SettingsAction::SaveAgentBorderLabels(enabled) => {
                    self.save_agent_border_labels(enabled)
                }
                SettingsAction::SaveSidebarTopNavigation(enabled) => {
                    self.save_sidebar_top_navigation(enabled)
                }
                SettingsAction::SaveSidebarHideActions(hidden) => {
                    self.save_sidebar_hide_actions(hidden)
                }
                SettingsAction::SaveSidebarHideFiles(hidden) => {
                    self.save_sidebar_hide_files(hidden)
                }
                SettingsAction::SaveSidebarHideBranch(hidden) => {
                    self.save_sidebar_hide_branch(hidden)
                }
            }
        }
        if self.state.agent_panel_scope != previous_agent_panel_scope {
            self.save_agent_panel_scope(self.state.agent_panel_scope);
        }

        if let Some(content) = self.state.request_clipboard_write.take() {
            if self
                .event_tx
                .try_send(crate::events::AppEvent::ClipboardWrite { content })
                .is_err()
            {
                tracing::warn!("failed to queue clipboard write event");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Mouse handling
// ---------------------------------------------------------------------------

// Note: split_pane needs runtime (event_tx for PTY spawn), so it lives on App
impl AppState {
    pub(crate) fn split_pane(&mut self, direction: Direction) {
        // Actual PTY spawning happens in Workspace::split_focused
        // which needs events channel — this is called from navigate_key
        // where we don't have async context, so the workspace handles it
        let (rows, cols) = self.estimate_pane_size();
        let new_rows = (rows / 2).max(4);
        let new_cols = (cols / 2).max(10);

        let cwd = self
            .active
            .and_then(|i| self.workspaces.get(i))
            .and_then(|ws| {
                let tab = ws.active_tab()?;
                tab.cwd_for_pane(
                    tab.layout.focused(),
                    &self.terminals,
                    &self.terminal_runtimes,
                )
            });

        if let Some(ws) = self.active.and_then(|i| self.workspaces.get_mut(i)) {
            if let Ok(new_pane) = ws.split_focused(
                direction,
                new_rows,
                new_cols,
                cwd,
                self.pane_scrollback_limit_bytes,
                self.host_terminal_theme,
            ) {
                let new_id = new_pane.pane_id;
                self.terminal_runtimes
                    .insert(new_pane.terminal.id.clone(), new_pane.runtime);
                self.terminals
                    .insert(new_pane.terminal.id.clone(), new_pane.terminal);
                ws.layout.focus_pane(new_id);
                self.mark_session_dirty();
                self.mode = Mode::Terminal;
            }
        }
    }

    /// Open a file from the filesystem panel by splitting the active
    /// workspace's focused pane with `$VISUAL`/`$EDITOR` (falling back to vi).
    pub(crate) fn open_path_in_pane(&mut self, path: &std::path::Path) {
        let Some(ws_idx) = self.active else {
            return;
        };
        let (rows, cols) = self.estimate_pane_size();
        let new_rows = (rows / 2).max(4);
        let new_cols = (cols / 2).max(10);

        let argv: Vec<String> = if crate::image_view::is_image_path(path) {
            // Images are unreadable in $EDITOR; re-invoke ourselves to
            // render them in the pane (external viewer or Kitty graphics).
            let exe = std::env::current_exe()
                .ok()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| "panels".to_string());
            let mut argv = vec![exe, "render-image".to_string()];
            // Tell the child whether the Kitty graphics relay is live so
            // it can pick in-pane rendering vs. opening the OS viewer.
            if crate::kitty_graphics::is_enabled() {
                argv.push("--kitty".to_string());
            }
            argv.push(path.to_string_lossy().into_owned());
            argv
        } else {
            let editor = std::env::var("VISUAL")
                .or_else(|_| std::env::var("EDITOR"))
                .unwrap_or_else(|_| "vi".to_string());
            let mut argv: Vec<String> = editor.split_whitespace().map(|s| s.to_string()).collect();
            if argv.is_empty() {
                argv.push("vi".to_string());
            }
            argv.push(path.to_string_lossy().into_owned());
            argv
        };

        let cwd = path.parent().map(|p| p.to_path_buf());

        let Some(ws) = self.workspaces.get_mut(ws_idx) else {
            return;
        };
        let Some(focused) = ws.focused_pane_id() else {
            return;
        };
        let Some(Ok((_, new_pane))) = ws.split_pane_argv_command(
            focused,
            Direction::Horizontal,
            new_rows,
            new_cols,
            cwd,
            &argv,
            self.pane_scrollback_limit_bytes,
            self.host_terminal_theme,
            true,
        ) else {
            return;
        };
        let new_id = new_pane.pane_id;
        self.terminal_runtimes
            .insert(new_pane.terminal.id.clone(), new_pane.runtime);
        self.terminals
            .insert(new_pane.terminal.id.clone(), new_pane.terminal);
        ws.layout.focus_pane(new_id);
        self.mark_session_dirty();
        self.mode = Mode::Terminal;
    }

    // -----------------------------------------------------------------
    // ACTIONS panel
    // -----------------------------------------------------------------

    /// Open the create/edit modal. `edit` is `Some(id)` to edit, `None`
    /// to create a new action.
    pub(crate) fn open_action_editor(&mut self, edit: Option<u64>) {
        use crate::app::state::{ActionDraft, ActionField};
        let draft = if let Some(id) = edit {
            let Some(a) = self.actions.iter().find(|a| a.id == id) else {
                return;
            };
            ActionDraft {
                editing_id: Some(id),
                name: a.name.clone(),
                command: a.command.clone(),
                field: ActionField::Name,
            }
        } else {
            ActionDraft {
                editing_id: None,
                name: String::new(),
                command: String::new(),
                field: ActionField::Name,
            }
        };
        self.action_draft = Some(draft);
        self.mode = Mode::ActionEditor;
    }

    pub(crate) fn cancel_action_editor(&mut self) {
        self.action_draft = None;
        modal::leave_modal(self);
    }

    pub(crate) fn commit_action_editor(&mut self) {
        use crate::app::state::{ActionItem, ActionStatus};
        let Some(draft) = self.action_draft.take() else {
            modal::leave_modal(self);
            return;
        };
        let name = draft.name.trim().to_string();
        let command = draft.command.trim().to_string();
        // A nameless action is meaningless; just close without saving.
        if name.is_empty() {
            modal::leave_modal(self);
            return;
        }
        if let Some(id) = draft.editing_id {
            if let Some(a) = self.actions.iter_mut().find(|a| a.id == id) {
                a.name = name;
                a.command = command;
                a.status = ActionStatus::Idle;
            }
        } else {
            let id = self.next_action_id;
            self.next_action_id += 1;
            self.actions.push(ActionItem {
                id,
                name,
                command,
                status: ActionStatus::Idle,
            });
        }
        self.mark_session_dirty();
        modal::leave_modal(self);
    }

    pub(crate) fn delete_action_in_editor(&mut self) {
        if let Some(draft) = self.action_draft.take() {
            if let Some(id) = draft.editing_id {
                self.actions.retain(|a| a.id != id);
                self.mark_session_dirty();
            }
        }
        modal::leave_modal(self);
    }

    /// Launch an action's command in a new split pane on the active
    /// workspace. If there is no active workspace/focused pane, or that
    /// pane is busy, the action is marked `NeedsTarget` instead.
    pub(crate) fn run_action(&mut self, id: u64) {
        use crate::app::state::ActionStatus;
        use crate::detect::AgentState;

        let Some(aidx) = self.actions.iter().position(|a| a.id == id) else {
            return;
        };
        let command = self.actions[aidx].command.trim().to_string();
        if command.is_empty() {
            self.actions[aidx].status = ActionStatus::NeedsTarget;
            return;
        }
        let Some(ws_idx) = self.active else {
            self.actions[aidx].status = ActionStatus::NeedsTarget;
            return;
        };
        let Some(focused) = self
            .workspaces
            .get(ws_idx)
            .and_then(|ws| ws.focused_pane_id())
        else {
            self.actions[aidx].status = ActionStatus::NeedsTarget;
            return;
        };
        let busy = self
            .workspaces
            .get(ws_idx)
            .map(|ws| {
                ws.pane_details(&self.terminals).into_iter().any(|d| {
                    d.pane_id == focused
                        && matches!(d.state, AgentState::Working | AgentState::Blocked)
                })
            })
            .unwrap_or(false);
        if busy {
            self.actions[aidx].status = ActionStatus::NeedsTarget;
            return;
        }

        let (rows, cols) = self.estimate_pane_size();
        let new_rows = (rows / 2).max(4);
        let new_cols = (cols / 2).max(10);
        let argv = vec!["sh".to_string(), "-lc".to_string(), command];
        let scrollback_limit = self.pane_scrollback_limit_bytes;
        let theme = self.host_terminal_theme;

        let split = {
            let Some(ws) = self.workspaces.get_mut(ws_idx) else {
                return;
            };
            ws.split_pane_argv_command(
                focused,
                Direction::Horizontal,
                new_rows,
                new_cols,
                None,
                &argv,
                scrollback_limit,
                theme,
                true,
            )
        };
        let Some(Ok((_, new_pane))) = split else {
            self.actions[aidx].status = ActionStatus::NeedsTarget;
            return;
        };
        let new_id = new_pane.pane_id;
        self.terminal_runtimes
            .insert(new_pane.terminal.id.clone(), new_pane.runtime);
        self.terminals
            .insert(new_pane.terminal.id.clone(), new_pane.terminal);
        if let Some(ws) = self.workspaces.get_mut(ws_idx) {
            ws.layout.focus_pane(new_id);
        }
        self.actions[aidx].status = ActionStatus::Running {
            ws_idx,
            pane_id: new_id,
        };
        self.mark_session_dirty();
        self.mode = Mode::Terminal;
    }
}

/// Local wall-clock time formatted as `HH:MM` for action completion
/// stamps. Falls back to `--:--` if the platform call fails.
pub(crate) fn local_hh_mm() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as libc::time_t;
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&secs, &mut tm).is_null() {
            return "--:--".to_string();
        }
        format!("{:02}:{:02}", tm.tm_hour, tm.tm_min)
    }
}

#[cfg(test)]
fn state_with_workspaces(names: &[&str]) -> AppState {
    let mut state = AppState::test_new();
    state.workspaces = names
        .iter()
        .map(|name| crate::workspace::Workspace::test_new(name))
        .collect();
    if !state.workspaces.is_empty() {
        state.active = Some(0);
        state.selected = 0;
        state.mode = Mode::Navigate;
    }
    state
}

#[cfg(test)]
fn app_for_mouse_test() -> App {
    let (_api_tx, api_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(
        &crate::config::Config::default(),
        true,
        None,
        None,
        api_rx,
        crate::api::EventHub::default(),
    );
    app.state.mode = Mode::Terminal;
    app.state.update_available = None;
    app.state.latest_release_notes_available = false;
    app.state.view.sidebar_rect = ratatui::layout::Rect::new(0, 0, 26, 20);
    app.state.view.terminal_area = ratatui::layout::Rect::new(26, 0, 80, 20);
    app
}

#[cfg(test)]
fn mouse(
    kind: crossterm::event::MouseEventKind,
    col: u16,
    row: u16,
) -> crossterm::event::MouseEvent {
    crossterm::event::MouseEvent {
        kind,
        column: col,
        row,
        modifiers: crossterm::event::KeyModifiers::empty(),
    }
}

#[cfg(test)]
fn numbered_lines_bytes(count: usize) -> Vec<u8> {
    (0..count)
        .map(|i| format!("{i:06}\r\n"))
        .collect::<String>()
        .into_bytes()
}

#[cfg(test)]
fn capture_snapshot(state: &AppState) -> crate::persist::SessionSnapshot {
    crate::persist::capture(
        &state.workspaces,
        &state.terminals,
        &state.terminal_runtimes,
        state.active,
        state.selected,
        state.agent_panel_scope,
        state.sidebar_width,
        state.sidebar_section_split,
        state.files_section_split,
        &state.actions,
    )
}

#[cfg(test)]
fn root_layout_ratio(snapshot: &crate::persist::SessionSnapshot) -> Option<f32> {
    match &snapshot.workspaces.first()?.tabs.first()?.layout {
        crate::persist::LayoutSnapshot::Split { ratio, .. } => Some(*ratio),
        crate::persist::LayoutSnapshot::Pane(_) => None,
    }
}

#[cfg(test)]
fn unique_temp_path(name: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("panels-{name}-{}-{nanos}", std::process::id()))
}

#[cfg(test)]
fn wait_for_file(path: &std::path::Path) -> String {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        if let Ok(content) = std::fs::read_to_string(path) {
            return content;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("timed out waiting for {}", path.display());
}

#[cfg(test)]
mod action_tests {
    use super::*;
    use crate::app::state::ActionStatus;

    #[test]
    fn commit_creates_then_edits_an_action() {
        let mut state = AppState::test_new();

        state.open_action_editor(None);
        assert_eq!(state.mode, Mode::ActionEditor);
        let d = state.action_draft.as_mut().unwrap();
        d.name = "clean diskspace".into();
        d.command = "  clean  ".into();
        state.commit_action_editor();

        assert_eq!(state.actions.len(), 1);
        assert_eq!(state.actions[0].name, "clean diskspace");
        assert_eq!(state.actions[0].command, "clean"); // trimmed
        assert!(state.action_draft.is_none());
        let id = state.actions[0].id;

        state.open_action_editor(Some(id));
        state.action_draft.as_mut().unwrap().command = "df -h".into();
        state.commit_action_editor();
        assert_eq!(state.actions.len(), 1);
        assert_eq!(state.actions[0].command, "df -h");
    }

    #[test]
    fn run_without_active_workspace_needs_target() {
        let mut state = AppState::test_new();
        state.open_action_editor(None);
        let d = state.action_draft.as_mut().unwrap();
        d.name = "build".into();
        d.command = "cargo build".into();
        state.commit_action_editor();
        let id = state.actions[0].id;

        // No active workspace -> cannot launch, surfaces "select active panel".
        state.active = None;
        state.run_action(id);
        assert_eq!(state.actions[0].status, ActionStatus::NeedsTarget);
    }

    #[test]
    fn delete_removes_the_action() {
        let mut state = AppState::test_new();
        state.open_action_editor(None);
        state.action_draft.as_mut().unwrap().name = "tmp".into();
        state.commit_action_editor();
        let id = state.actions[0].id;

        state.open_action_editor(Some(id));
        state.delete_action_in_editor();
        assert!(state.actions.is_empty());
        assert!(state.action_draft.is_none());
    }
}
