use super::*;
use crate::history_cell::HistoryCell;
use crate::history_cell::new_user_prompt;
use chrono::TimeZone;
use pretty_assertions::assert_eq;

#[test]
fn timestamp_dividers_fit_terminal_widths() {
    // Build local wall time directly so the snapshot is independent of the host zone.
    let timestamp = Local
        .with_ymd_and_hms(
            /*year*/ 2026, /*month*/ 10, /*day*/ 10, /*hour*/ 14,
            /*min*/ 2, /*sec*/ 18,
        )
        .unwrap();
    let mut previews = Vec::new();
    for width in [80, 48, 24, 18, 8, 4, 1, 0] {
        let lines = divider(timestamp, width);
        assert!(lines.iter().all(|line| line.width() <= usize::from(width)));
        previews.push(format!(
            "width {width}\n{}",
            ratatui::text::Text::from(crate::terminal_hyperlinks::visible_lines(lines))
        ));
    }
    insta::assert_snapshot!(previews.join("\n\n"));
}

#[test]
fn timestamp_is_display_only_and_preserves_wrapped_links() {
    let mut cell = new_user_prompt(
        "Check https://example.test/wrapped-prompt and keep the link intact.".into(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let original = cell.raw_lines();
    cell.timestamp = Some(
        Local
            .with_ymd_and_hms(
                /*year*/ 2026, /*month*/ 10, /*day*/ 10, /*hour*/ 14,
                /*min*/ 6, /*sec*/ 42,
            )
            .unwrap(),
    );
    let mut previews = Vec::new();
    for width in [80, 40, 20] {
        let lines = cell.display_hyperlink_lines(width);
        assert!(lines.iter().all(|line| line.width() <= usize::from(width)));
        assert!(lines[0].source.is_none());
        assert!(lines[0].hyperlinks.is_empty());
        assert_eq!(cell.raw_lines(), original);
        assert_eq!(cell.transcript_hyperlink_lines(width), lines);
        let linked = lines
            .iter()
            .flat_map(|line| {
                line.hyperlinks.iter().map(move |link| {
                    line.line
                        .to_string()
                        .chars()
                        .skip(link.columns.start)
                        .take(link.columns.len())
                        .collect::<String>()
                })
            })
            .collect::<String>();
        assert_eq!(linked, "https://example.test/wrapped-prompt");
        previews.push(format!(
            "width {width}\n{}",
            ratatui::text::Text::from(crate::terminal_hyperlinks::visible_lines(lines))
        ));
    }
    insta::assert_snapshot!(previews.join("\n\n"));
}

#[test]
fn missing_or_invalid_timestamp_has_no_divider() {
    let plain = new_user_prompt("Question".into(), Vec::new(), Vec::new(), Vec::new());
    for timestamp in [None, Some(0), Some(-1), Some(i64::MAX)] {
        let cell = new_user_prompt("Question".into(), Vec::new(), Vec::new(), Vec::new())
            .with_timestamp(timestamp);
        assert_eq!(
            cell.display_lines(/*width*/ 40),
            plain.display_lines(/*width*/ 40)
        );
    }
    let empty = new_user_prompt(String::new(), Vec::new(), Vec::new(), Vec::new())
        .with_timestamp(Some(1_791_640_938_000));
    assert_eq!(empty.display_lines(/*width*/ 40), Vec::<Line>::new());
}
