use crate::history_cell::AgentMarkdownCell;
use crate::history_cell::HistoryCell;
use crate::terminal_hyperlinks::visible_lines;
use codex_protocol::models::MessagePhase;
use pretty_assertions::assert_eq;
use ratatui::style::Color;
use ratatui::style::Stylize;
use ratatui::text::Line;
use std::path::Path;

const ANSWER: &str = "Updated the final-message display.\n\n- Final answers are easier to find.\n- Commentary stays unchanged.\n- Nothing was committed or installed.\n\n**Checks:** 12 passed.\n\nSee [the patch](https://example.test/patch).";
const RICH_ANSWER: &str = "## Result\n\nThe display now keeps the message phase. Long final answers wrap at the terminal width without losing their links or markdown.\n\n```rust\nlet phase = MessagePhase::FinalAnswer;\n```\n\n| Item | Result |\n| --- | --- |\n| Final answer | Clear |\n| Commentary | Unchanged |\n\nSee [the full review](https://example.test/review).";

#[test]
fn final_message_top_separator_layout() {
    let mut previews = Vec::new();
    for width in [80, 48, 24] {
        for (sample, source) in [("short", ANSWER), ("rich", RICH_ANSWER)] {
            let cell = AgentMarkdownCell::new(source.into(), Path::new("/tmp"))
                .with_message_phase(Some(MessagePhase::FinalAnswer));
            let lines = cell.display_hyperlink_lines(width);
            assert!(lines.iter().all(|line| line.width() <= usize::from(width)));
            assert_eq!(cell.transcript_hyperlink_lines(width), lines);
            assert_eq!(
                cell.raw_lines(),
                AgentMarkdownCell::new(source.into(), Path::new("/tmp")).raw_lines()
            );
            assert!(lines.iter().any(|line| !line.hyperlinks.is_empty()));
            let text = ratatui::text::Text::from(visible_lines(lines)).to_string();
            let text = text
                .lines()
                .map(str::trim_end)
                .collect::<Vec<_>>()
                .join("\n");
            previews.push(format!("{sample}, width {width}\n{text}"));
        }
    }
    insta::assert_snapshot!(previews.join("\n\n"));
}

#[test]
fn final_message_top_separator_preserves_hyperlink_ranges_and_resize() {
    let cell = AgentMarkdownCell::new(
        "[Review](https://example.test/review) and 中文 café".into(),
        Path::new("/tmp"),
    )
    .with_message_phase(Some(MessagePhase::FinalAnswer));
    for width in [80, 24, 18, 17, 10] {
        let lines = cell.display_hyperlink_lines(width);
        assert!(lines.iter().all(|line| line.width() <= usize::from(width)));
        let links: Vec<_> = lines
            .iter()
            .flat_map(|line| {
                line.hyperlinks.iter().map(move |link| {
                    (
                        line.line
                            .to_string()
                            .chars()
                            .skip(link.columns.start)
                            .take(link.columns.len())
                            .collect::<String>(),
                        link.destination.clone(),
                    )
                })
            })
            .collect();
        assert!(
            links
                .iter()
                .all(|(_, destination)| destination == "https://example.test/review")
        );
        assert_eq!(
            links.into_iter().map(|(text, _)| text).collect::<String>(),
            "Reviewhttps://example.test/review"
        );
    }
}

#[test]
fn only_explicit_final_answers_get_a_treatment() {
    let plain = AgentMarkdownCell::new(ANSWER.into(), Path::new("/tmp"));
    for phase in [None, Some(MessagePhase::Commentary)] {
        let cell =
            AgentMarkdownCell::new(ANSWER.into(), Path::new("/tmp")).with_message_phase(phase);
        assert_eq!(
            cell.display_lines(/*width*/ 48),
            plain.display_lines(/*width*/ 48)
        );
    }
    let cell = AgentMarkdownCell::new(ANSWER.into(), Path::new("/tmp"))
        .with_message_phase(Some(MessagePhase::FinalAnswer));
    assert!(cell.final_answer);
    assert_eq!(
        cell.display_lines(/*width*/ 48)[0],
        Line::from("─".repeat(/*n*/ 48)).magenta()
    );
}

#[test]
fn final_message_color_changes_only_the_top_separator() {
    let color = Color::Rgb(137, 180, 250);
    let default = AgentMarkdownCell::new(ANSWER.into(), Path::new("/tmp"))
        .with_message_phase(Some(MessagePhase::FinalAnswer));
    let custom = AgentMarkdownCell::new(ANSWER.into(), Path::new("/tmp"))
        .with_message_phase(Some(MessagePhase::FinalAnswer))
        .with_final_message_color(color);
    let mut expected = default.display_hyperlink_lines(/*width*/ 48);
    expected[0].line = expected[0].line.clone().fg(color);
    let actual = custom.display_hyperlink_lines(/*width*/ 48);
    assert_eq!(actual, expected);
    assert_eq!(custom.raw_lines(), default.raw_lines());
    insta::assert_debug_snapshot!(actual[..2]);
    let commentary = AgentMarkdownCell::new(ANSWER.into(), Path::new("/tmp"))
        .with_message_phase(Some(MessagePhase::Commentary))
        .with_final_message_color(color);
    assert_eq!(
        commentary.display_lines(/*width*/ 48),
        AgentMarkdownCell::new(ANSWER.into(), Path::new("/tmp")).display_lines(/*width*/ 48)
    );
}

#[test]
fn final_message_uses_the_full_width_even_in_tiny_terminals() {
    let cell = AgentMarkdownCell::new("```\na | b\n```".into(), Path::new("/tmp"))
        .with_message_phase(Some(MessagePhase::FinalAnswer));
    assert_eq!(cell.display_lines(/*width*/ 0), Vec::<Line>::new());
    let mut previews = Vec::new();
    for width in [1, 2, 3, 5] {
        let lines = cell.display_lines(width);
        assert!(lines.iter().all(|line| line.width() <= usize::from(width)));
        previews.push(format!(
            "width {width}\n{}",
            ratatui::text::Text::from(lines)
        ));
    }
    insta::assert_snapshot!(previews.join("\n\n"));
}

#[test]
fn final_message_native_scrollback_keeps_separator_above_clean_body() {
    use crate::insert_history::HistoryLineWrapPolicy;
    use crate::insert_history::InsertHistoryMode;
    use crate::insert_history::insert_history_hyperlink_lines_with_mode_and_wrap_policy;
    use crate::test_backend::VT100Backend;
    use ratatui::layout::Rect;
    use ratatui::layout::Size;

    let cell = AgentMarkdownCell::new("```\n    left | right\n```".into(), Path::new("/tmp"))
        .with_message_phase(Some(MessagePhase::FinalAnswer));
    for mode in [InsertHistoryMode::Standard, InsertHistoryMode::FullScreen] {
        for wrap in [
            HistoryLineWrapPolicy::PreWrap,
            HistoryLineWrapPolicy::Terminal,
        ] {
            let backend = VT100Backend::new(/*width*/ 32, /*height*/ 8);
            let mut terminal = crate::custom_terminal::Terminal::with_options(backend).unwrap();
            terminal.set_viewport_area(Rect::new(
                /*x*/ 0, /*y*/ 7, /*width*/ 32, /*height*/ 1,
            ));
            insert_history_hyperlink_lines_with_mode_and_wrap_policy(
                &mut terminal,
                &cell.display_hyperlink_lines(/*width*/ 32),
                mode,
                wrap,
                Size::new(/*width*/ 32, /*height*/ 8),
            )
            .unwrap();
            // A terminal-owned selection can include the separator's actual screen cells.
            assert_eq!(
                terminal.backend().vt100().screen().contents().trim(),
                format!("{}\n    left | right", "─".repeat(/*n*/ 32))
            );
        }
    }
}
