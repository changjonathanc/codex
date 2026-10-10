//! Display-only user-message timestamps. Prompt source text is unchanged.

use super::UserHistoryCell;
use crate::live_wrap::take_prefix_by_width;
use crate::terminal_hyperlinks::HyperlinkLine;
use chrono::DateTime;
use chrono::Local;
use ratatui::style::Stylize;
use ratatui::text::Line;
use unicode_width::UnicodeWidthStr;

impl UserHistoryCell {
    pub(crate) fn with_timestamp(mut self, timestamp_ms: Option<i64>) -> Self {
        self.timestamp = timestamp_ms
            .filter(|timestamp| *timestamp > 0)
            .and_then(DateTime::from_timestamp_millis)
            .map(|timestamp| timestamp.with_timezone(&Local));
        self
    }
}

pub(super) fn divider(timestamp: DateTime<Local>, width: u16) -> Vec<HyperlinkLine> {
    if width == 0 {
        return Vec::new();
    }
    let date_time = timestamp.format("%d %b · %H:%M").to_string();
    let label = if usize::from(width) >= date_time.width() + 6 {
        date_time
    } else {
        timestamp.format("%H:%M").to_string()
    };
    let (label, _, _) = take_prefix_by_width(&label, usize::from(width));
    let remaining = usize::from(width).saturating_sub(label.width());
    let text = if remaining >= 4 {
        let rail = remaining - 2;
        let left = "─".repeat(rail / 2);
        let right = "─".repeat(rail - rail / 2);
        format!("{left} {label} {right}")
    } else {
        label
    };
    vec![HyperlinkLine::new(Line::from(text.dim()))]
}

#[cfg(test)]
#[path = "user_message_timestamp_tests.rs"]
mod tests;
