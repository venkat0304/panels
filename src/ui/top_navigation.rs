use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::Paragraph,
    Frame,
};

use super::widgets::panel_contrast_fg;
use crate::app::{state::WorkspaceCardArea, AppState, Mode};

const MIN_WORKSPACE_TAB_WIDTH: u16 = 8;
const ACTIONS_BUTTON_WIDTH: u16 = 7;
const SETTINGS_BUTTON_WIDTH: u16 = 8;
const NEW_WORKSPACE_BUTTON_WIDTH: u16 = 1;

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

    let visible_count = workspace_count.min(area.width as usize);
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
            (label.chars().count() as u16 + 2).max(MIN_WORKSPACE_TAB_WIDTH)
        })
        .collect::<Vec<_>>();
    let desired_total = desired_widths
        .iter()
        .copied()
        .fold(0u16, u16::saturating_add);
    let widths = if desired_total <= area.width {
        desired_widths
    } else {
        let base = area.width / visible_count as u16;
        let remainder = area.width % visible_count as u16;
        (0..visible_count)
            .map(|idx| base + u16::from((idx as u16) < remainder))
            .collect()
    };

    let mut x = area.x;
    for (offset, width) in widths.into_iter().enumerate() {
        areas[start + offset].rect = Rect::new(x, area.y, width, 1);
        x = x.saturating_add(width);
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
        frame.render_widget(Paragraph::new(format!(" {name} ")).style(style), tab.rect);
    }

    let controls = top_navigation_areas(area);
    render_control(
        frame,
        controls.actions,
        "actions",
        app.mode == Mode::ActionsMenu,
        app,
    );
    render_control(
        frame,
        controls.settings,
        "settings",
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
    frame.render_widget(Paragraph::new(label).style(style), area);
}
