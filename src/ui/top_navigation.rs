use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    widgets::Paragraph,
    Frame,
};

use super::widgets::panel_contrast_fg;
use crate::app::{state::WorkspaceCardArea, AppState, Mode};

const MIN_WORKSPACE_TAB_WIDTH: u16 = 10;
const WORKSPACE_TAB_GAP: u16 = 1;
const ACTIONS_BUTTON_WIDTH: u16 = 9;
const SETTINGS_BUTTON_WIDTH: u16 = 10;
const NEW_WORKSPACE_BUTTON_WIDTH: u16 = 3;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TopNavigationAreas {
    pub workspace_tabs: Rect,
    pub actions: Rect,
    pub settings: Rect,
    pub new_workspace: Rect,
}

pub(crate) fn top_navigation_areas(area: Rect) -> TopNavigationAreas {
    if area.width == 0 || area.height == 0 {
        return TopNavigationAreas::default();
    }

    let left = area.x;
    let mut right = area.x.saturating_add(area.width);
    let new_workspace = take_right(left, &mut right, area.y, NEW_WORKSPACE_BUTTON_WIDTH);
    reserve_gap(left, &mut right);
    let settings = take_right(left, &mut right, area.y, SETTINGS_BUTTON_WIDTH);
    reserve_gap(left, &mut right);
    let actions = take_right(left, &mut right, area.y, ACTIONS_BUTTON_WIDTH);
    reserve_gap(left, &mut right);

    TopNavigationAreas {
        workspace_tabs: Rect::new(left, area.y, right.saturating_sub(left), 1),
        actions,
        settings,
        new_workspace,
    }
}

fn take_right(left: u16, right: &mut u16, y: u16, desired: u16) -> Rect {
    let width = desired.min(right.saturating_sub(left));
    *right = right.saturating_sub(width);
    Rect::new(*right, y, width, 1)
}

fn reserve_gap(left: u16, right: &mut u16) {
    *right = right.saturating_sub(u16::from(*right > left));
}

pub(crate) fn compute_workspace_tab_areas(app: &AppState, area: Rect) -> Vec<WorkspaceCardArea> {
    let workspace_count = app.workspaces.len();
    let mut areas = (0..workspace_count)
        .map(|ws_idx| WorkspaceCardArea {
            ws_idx,
            rect: Rect::default(),
        })
        .collect::<Vec<_>>();
    if workspace_count == 0 || area.width == 0 || area.height == 0 {
        return areas;
    }

    let max_visible_count = ((area.width.saturating_add(WORKSPACE_TAB_GAP))
        / MIN_WORKSPACE_TAB_WIDTH.saturating_add(WORKSPACE_TAB_GAP))
    .max(1) as usize;
    let visible_count = workspace_count.min(max_visible_count);
    let active = app.active.unwrap_or(app.selected).min(workspace_count - 1);
    let start = if visible_count < workspace_count {
        active
            .saturating_sub(visible_count / 2)
            .min(workspace_count - visible_count)
    } else {
        0
    };
    let end = start + visible_count;

    let desired_widths = (start..end)
        .map(|idx| {
            let label =
                app.workspaces[idx].display_name_from(&app.terminals, &app.terminal_runtimes);
            (label.chars().count() as u16 + 5).max(MIN_WORKSPACE_TAB_WIDTH)
        })
        .collect::<Vec<_>>();
    let gap_total = WORKSPACE_TAB_GAP.saturating_mul(visible_count.saturating_sub(1) as u16);
    let width_budget = area.width.saturating_sub(gap_total);
    let desired_total = desired_widths
        .iter()
        .copied()
        .fold(0u16, u16::saturating_add);
    let widths = if desired_total <= width_budget {
        desired_widths
    } else {
        let base = width_budget / visible_count as u16;
        let remainder = width_budget % visible_count as u16;
        (0..visible_count)
            .map(|idx| base + u16::from((idx as u16) < remainder))
            .collect()
    };

    let mut x = area.x;
    for (offset, width) in widths.into_iter().enumerate() {
        areas[start + offset].rect = Rect::new(x, area.y, width, 1);
        x = x.saturating_add(width);
        if offset + 1 < visible_count {
            x = x.saturating_add(WORKSPACE_TAB_GAP);
        }
    }

    areas
}

pub(super) fn render_workspace_tabs(app: &AppState, frame: &mut Frame, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let p = &app.palette;
    frame.render_widget(
        Paragraph::new(" ".repeat(area.width as usize)).style(Style::default().bg(p.panel_bg)),
        area,
    );

    for tab in &app.view.workspace_card_areas {
        if tab.rect.width == 0 {
            continue;
        }
        let active = app.active == Some(tab.ws_idx);
        let selected = app.mode == Mode::Navigate && app.selected == tab.ws_idx;
        let style = if active {
            Style::default()
                .fg(panel_contrast_fg(p))
                .bg(p.accent)
                .add_modifier(Modifier::BOLD)
        } else if selected {
            Style::default()
                .fg(p.text)
                .bg(p.surface1)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(p.subtext0).bg(p.surface0)
        };
        let name =
            app.workspaces[tab.ws_idx].display_name_from(&app.terminals, &app.terminal_runtimes);
        let marker = if active {
            "● "
        } else if selected {
            "› "
        } else {
            ""
        };
        frame.render_widget(
            Paragraph::new(workspace_tab_label(&name, marker, tab.rect.width))
                .style(style)
                .alignment(Alignment::Center),
            tab.rect,
        );
    }

    let controls = top_navigation_areas(area);
    render_control(
        frame,
        controls.actions,
        "Actions",
        app.mode == Mode::ActionsMenu,
        app,
    );
    render_control(
        frame,
        controls.settings,
        "Settings",
        app.mode == Mode::GlobalMenu,
        app,
    );
    render_control(frame, controls.new_workspace, "+", false, app);
}

fn render_control(frame: &mut Frame, area: Rect, label: &str, active: bool, app: &AppState) {
    if area.width == 0 {
        return;
    }
    let style = if active {
        Style::default()
            .fg(panel_contrast_fg(&app.palette))
            .bg(app.palette.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(app.palette.overlay1)
            .bg(app.palette.surface0)
    };
    frame.render_widget(
        Paragraph::new(label)
            .style(style)
            .alignment(Alignment::Center),
        area,
    );
}

fn workspace_tab_label(name: &str, marker: &str, width: u16) -> String {
    let content_width = (width as usize).saturating_sub(2);
    let marker_width = marker.chars().count().min(content_width);
    if marker_width == content_width {
        return marker.chars().take(content_width).collect();
    }

    let name_width = content_width - marker_width;
    let name = truncate_with_ellipsis(name, name_width);
    format!(
        "{}{name}",
        marker.chars().take(marker_width).collect::<String>()
    )
}

fn truncate_with_ellipsis(value: &str, width: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= width {
        return value.to_string();
    }
    if width == 0 {
        return String::new();
    }
    if width == 1 {
        return "…".to_string();
    }

    format!("{}…", value.chars().take(width - 1).collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::{truncate_with_ellipsis, workspace_tab_label};

    #[test]
    fn workspace_labels_keep_breathing_room_and_truncate_cleanly() {
        assert_eq!(workspace_tab_label("space 1", "● ", 12), "● space 1");
        assert_eq!(workspace_tab_label("a very long space", "", 10), "a very …");
        assert_eq!(truncate_with_ellipsis("workspace", 1), "…");
    }
}
