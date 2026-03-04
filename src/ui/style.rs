use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

pub fn styled_log_line(s: &str) -> Line<'_> {
    let (color, style_span) = if s.contains("错误") || s.to_lowercase().contains("error") {
        (Color::Red, true)
    } else if s.contains("更优") || s.contains("成功") || s.contains("最佳") || s.contains("完成")
    {
        (Color::Green, true)
    } else if s.contains("速度") {
        (Color::Cyan, false)
    } else if s.contains("延迟") {
        (Color::Magenta, false)
    } else {
        (Color::Gray, false)
    };

    let style = if style_span {
        Style::default().fg(color).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(color)
    };
    Line::from(Span::styled(s.to_string(), style))
}
