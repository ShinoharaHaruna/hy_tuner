use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, List, ListItem};

use crate::config::Tunable;
use crate::ui::style::styled_log_line;
use crate::ui::types::{Field, Focus};

pub fn render_ui(
    f: &mut Frame,
    state: &Tunable,
    fields: &[Field],
    selected: usize,
    focus: Focus,
    logs: &[String],
    log_scroll: usize,
) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(f.area());

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(main_chunks[0]);

    render_params_panel(f, chunks[0], state, fields, selected, focus);
    render_logs_panel(f, chunks[1], logs, log_scroll, focus);
    render_help_panel(f, main_chunks[1]);
}

fn render_params_panel(
    f: &mut Frame,
    area: ratatui::layout::Rect,
    state: &Tunable,
    fields: &[Field],
    selected: usize,
    focus: Focus,
) {
    let items: Vec<ListItem> = fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let name = match field {
                Field::TestFileUrl => format!("测速文件: {}", state.test_file_url),
                Field::LatencyUrl => format!("延迟探针: {}", state.latency_url),
                Field::MinUp => format!("上行最小: {} Mbps", state.min_up),
                Field::MaxUp => format!("上行最大: {} Mbps", state.max_up),
                Field::MinDown => format!("下行最小: {} Mbps", state.min_down),
                Field::MaxDown => format!("下行最大: {} Mbps", state.max_down),
                Field::TargetAccuracy => {
                    format!("搜索精度: {} Mbps", state.target_accuracy)
                }
            };
            let style = if i == selected && focus == Focus::Params {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(ratatui::style::Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(name).style(style)
        })
        .collect();

    let list = List::new(items).block(Block::default().borders(Borders::ALL).title("参数"));
    f.render_widget(list, area);
}

fn render_logs_panel(
    f: &mut Frame,
    area: ratatui::layout::Rect,
    logs: &[String],
    log_scroll: usize,
    focus: Focus,
) {
    let max_visible = area.height.saturating_sub(2) as usize;
    let start = logs.len().saturating_sub(max_visible + log_scroll);
    let end = logs.len().saturating_sub(log_scroll);
    let slice = if start < end && end <= logs.len() {
        &logs[start..end]
    } else {
        &[]
    };
    let log_items: Vec<ListItem> = slice
        .iter()
        .map(|l| ListItem::new(styled_log_line(l)))
        .collect();
    let border_style = if focus == Focus::Logs {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let log_list = List::new(log_items).block(
        Block::default()
            .borders(Borders::ALL)
            .style(border_style)
            .title("日志"),
    );
    f.render_widget(log_list, area);
}

fn render_help_panel(f: &mut Frame, area: ratatui::layout::Rect) {
    let help_text = vec![
        "Tab: 切换焦点 | ↑↓: 选择选项 | ←→: 调整数值 | e: 编辑 | s: 开始调优 | q: 退出",
        "日志区域: ↑↓/PgUp/PgDn: 滚动",
    ];
    let help_items: Vec<ListItem> = help_text.iter().map(|line| ListItem::new(*line)).collect();
    let help_list = List::new(help_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("帮助")
            .style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(help_list, area);
}
