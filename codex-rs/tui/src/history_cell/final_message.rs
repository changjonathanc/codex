//! Final-answer top separators. Markdown, links and selectable source text stay intact.

use codex_config::types::FinalMessageColor;
use ratatui::style::Color;
use ratatui::style::Stylize;

use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::LineDecoration;
use crate::terminal_hyperlinks::LogicalLineSource;
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
    if width == 0 || lines.is_empty() {
        return Vec::new();
    }
    // Markdown can keep a long URL on one row. Break only oversized rows so a final
    // answer fits without clipping. OSC 8 targets and copy provenance remain intact.
    let body_width = usize::from(width);
    let decoration = LineDecoration::HorizontalRule;
    // Owned transcript selection excludes this row by metadata. Native terminal selection
    // still sees its screen cells, so the answer body must remain free of decoration.
    let separator = HyperlinkLine {
        line: decoration.render(width).fg(color),
        source: Some(LogicalLineSource {
            decoration: Some(decoration),
            ..LogicalLineSource::new(String::new())
        }),
        ..Default::default()
    };
    std::iter::once(separator)
        .chain(lines.into_iter().flat_map(|line| {
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
        }))
        .collect()
}
