use super::*;
use crate::clipboard_copy::CopyFormat;
use crate::clipboard_copy::CopyStatus;
use crate::history_cell::AgentMarkdownCell;
use crate::transcript_view::tests::render;
use codex_protocol::models::MessagePhase;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use crossterm::event::MouseButton;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use pretty_assertions::assert_eq;
use std::path::Path;

const ANSWER: &str = "Run **this**:\n\n```sh\n    rg foo input | sort\n┃ literal | text\n────────\n```\n\nSee [review](https://example.test/review).\n\n---\n\nLast.";

#[test]
fn final_answer_selection_copies_content_across_separator_and_resize() {
    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(
        AgentMarkdownCell::new(ANSWER.into(), Path::new("/"))
            .with_message_phase(Some(MessagePhase::FinalAnswer)),
    )];
    let plain_cell = AgentMarkdownCell::new(ANSWER.into(), Path::new("/"));
    let plain = TextLayout::new(
        plain_cell.retained_hyperlink_lines(/*width*/ 80, /*detailed*/ false),
        /*width*/ 80,
    );
    let mut expected_lines = Vec::new();
    plain.copy_lines(0..plain.text().len(), "", &mut expected_lines);
    let expected = crate::markdown_copy::selection(&expected_lines, plain.text());
    assert_eq!(
        crate::clipboard_html::render_markdown(&expected.0),
        "<p>Run <strong>this</strong>:</p>\n<pre><code>    rg foo input | sort\n┃ literal | text\n────────\n</code></pre>\n<p>See <a href=\"https://example.test/review\">review (https://example.test/review)</a>.</p>\n<hr />\n<p>Last.</p>\n"
    );

    for detailed in [false, true] {
        let mut view = TranscriptView {
            detailed,
            copy_on_select: true,
            ..Default::default()
        };
        render(&mut view, &cells, /*width*/ 80, /*height*/ 256);
        // Begin on the decoration and release beyond the last body character.
        for (kind, column, row) in [
            (MouseEventKind::Down(MouseButton::Left), 0, 0),
            (MouseEventKind::Drag(MouseButton::Left), 80, 255),
        ] {
            assert!(matches!(
                view.handle_mouse(
                    MouseEvent {
                        kind,
                        column,
                        row,
                        modifiers: KeyModifiers::NONE
                    },
                    &cells
                ),
                Some(ViewAction::Changed)
            ));
        }
        let action = view.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Up(MouseButton::Left),
                column: 80,
                row: 255,
                modifiers: KeyModifiers::NONE,
            },
            &cells,
        );
        let Some(ViewAction::CopyOnSelect(selected)) = action else {
            panic!("selection release must request a copy");
        };
        assert_eq!(selected, plain.text());

        for width in [80, 24, 10, 2, 1] {
            let fresh = TextLayout::new(cells[0].retained_hyperlink_lines(width, detailed), width);
            assert_eq!(fresh.text(), plain.text());
            let mut fresh_lines = Vec::new();
            fresh.copy_lines(0..fresh.text().len(), "", &mut fresh_lines);
            assert_eq!(
                crate::markdown_copy::selection(&fresh_lines, fresh.text()),
                expected
            );
            let buffer = render(&mut view, &cells, width, /*height*/ 256);
            assert_eq!(view.selected_text(&cells).as_deref(), Some(plain.text()));
            let layout = view.layout(&cells, /*index*/ 0).unwrap();
            assert_eq!(layout.position_at(/*row*/ 0, width), 0);
            // The separator is drawn but has no selectable cells or hyperlink.
            for column in 0..width {
                assert_eq!(buffer[(column, 0)].symbol(), "─");
                assert!(
                    !buffer[(column, 0)]
                        .modifier
                        .contains(ratatui::style::Modifier::REVERSED)
                );
                assert_eq!(view.link_at(column, /*row*/ 0), None);
            }
            let Some(ViewAction::Copy(text)) = view.handle_key(
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                &cells,
            ) else {
                panic!("copy key must use the selected content");
            };
            view.copy_selected_text_with(
                &cells,
                &text,
                /*clear_selection*/ false,
                |text, format| {
                    assert_eq!(text, plain.text());
                    assert_eq!(
                        format,
                        CopyFormat::MarkdownSelection(expected.0.clone().into())
                    );
                    Ok(CopyStatus::Confirmed)
                },
            )
            .unwrap();
        }
    }
}

#[test]
fn final_answer_body_selection_preserves_links() {
    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(
        AgentMarkdownCell::new(
            "[review](https://example.test/review)\n\n```\n    left | right\n┃ real bar\n```"
                .into(),
            Path::new("/"),
        )
        .with_message_phase(Some(MessagePhase::FinalAnswer)),
    )];
    for width in [80, 24, 10] {
        let mut view = TranscriptView::default();
        render(&mut view, &cells, width, /*height*/ 40);
        let down = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 0,
            row: 1,
            modifiers: KeyModifiers::NONE,
        };
        view.handle_mouse(down, &cells);
        assert!(matches!(view.handle_mouse(MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            ..down
        }, &cells), Some(ViewAction::OpenLink(url)) if url == "https://example.test/review"));

        view.begin_selection(&cells, /*column*/ 0, /*row*/ 1, /*clicks*/ 1);
        view.extend_selection(/*column*/ 6, /*row*/ 1);
        let selected = view.selected_text(&cells).unwrap();
        assert_eq!(selected, "review");
        view.copy_selected_text_with(
            &cells,
            &selected,
            /*clear_selection*/ true,
            |text, format| {
                assert_eq!(text, "review");
                assert_eq!(
                    format,
                    CopyFormat::MarkdownSelection("[review](<https://example.test/review>)".into())
                );
                Ok(CopyStatus::Confirmed)
            },
        )
        .unwrap();
    }
}

#[test]
fn final_answer_selection_crosses_cells_without_copying_decorations() {
    let cells: Vec<Arc<dyn HistoryCell>> = ["first | answer", "second ┃ answer"]
        .into_iter()
        .map(|source| {
            Arc::new(
                AgentMarkdownCell::new(source.into(), Path::new("/"))
                    .with_message_phase(Some(MessagePhase::FinalAnswer)),
            ) as Arc<dyn HistoryCell>
        })
        .collect();
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 24, /*height*/ 12);
    view.begin_selection(&cells, /*column*/ 0, /*row*/ 0, /*clicks*/ 1);
    view.extend_selection(/*column*/ 24, /*row*/ 0);
    assert_eq!(view.selected_text(&cells), None);
    view.extend_selection(/*column*/ 24, /*row*/ 11);
    assert_eq!(
        view.selected_text(&cells).as_deref(),
        Some("first | answer\nsecond ┃ answer")
    );
}

#[test]
fn final_answer_raw_selection_keeps_original_markdown() {
    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(
        AgentMarkdownCell::new(ANSWER.into(), Path::new("/"))
            .with_message_phase(Some(MessagePhase::FinalAnswer)),
    )];
    let mut view = TranscriptView {
        mode: HistoryRenderMode::Raw,
        ..Default::default()
    };
    render(&mut view, &cells, /*width*/ 24, /*height*/ 80);
    view.begin_selection(&cells, /*column*/ 0, /*row*/ 0, /*clicks*/ 1);
    view.extend_selection(/*column*/ 24, /*row*/ 79);
    let selected = view.selected_text(&cells).unwrap();
    assert_eq!(selected, ANSWER);
    view.copy_selected_text_with(
        &cells,
        &selected,
        /*clear_selection*/ true,
        |text, format| {
            assert_eq!((text, format), (ANSWER, CopyFormat::PlainText));
            Ok(CopyStatus::Confirmed)
        },
    )
    .unwrap();
}
