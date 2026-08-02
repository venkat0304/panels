use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::Paragraph,
    Frame,
};

use super::widgets::panel_contrast_fg;
use crate::app::{state::WorkspaceCardArea, AppState, Mode};

const MIN_WORKSPACE_TAB_WIDTH: u16 = 8;

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
}
