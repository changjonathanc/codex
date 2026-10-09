//! Terminal-only treatments for completed final answers. Markdown and links stay intact.

use codex_config::types::FinalMessageColor;
use ratatui::style::Color;
use ratatui::style::Stylize;

use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::prefix_hyperlink_lines;
use crate::terminal_hyperlinks::remap_source_wrapped_line;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_line_with_source;

pub(crate) fn configured_color(setting: Option<FinalMessageColor>) -> Color {
    setting.map_or(Color::Magenta, |color| {
        let [red, green, blue] = color.rgb();
        crate::style::readable_color_on(Color::Rgb(red, green, blue), /*background*/ None)
    })
}

pub(crate) fn render(lines: Vec<HyperlinkLine>, width: u16, color: Color) -> Vec<HyperlinkLine> {
    // Markdown can keep a long URL on one row. Break only oversized rows so a final
    // answer, including its border, fits without clipping. OSC 8 targets remain intact.
    let body_width = usize::from(width.saturating_sub(2)).max(1);
    let lines = lines
        .into_iter()
        .flat_map(|line| {
            if line.width() <= body_width {
                vec![line]
            } else {
                remap_source_wrapped_line(
                    &line,
                    word_wrap_line_with_source(
                        &line.line,
                        RtOptions::new(body_width).subsequent_indent(
                            crate::insert_history::leading_whitespace_prefix(&line.line),
                        ),
                    ),
                )
            }
        })
        .collect();
    prefix_hyperlink_lines(lines, "┃ ".fg(color), "┃ ".fg(color))
}
