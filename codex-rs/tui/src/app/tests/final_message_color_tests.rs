use super::*;
use codex_config::types::FinalMessageColor;
use codex_protocol::models::MessagePhase;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn final_message_color_is_used_for_live_consolidation() -> Result<()> {
    let mut app = make_test_app().await;
    let color = FinalMessageColor::try_from("#89b4fa".to_string()).unwrap();
    app.local_settings.tui.final_message_color = Some(color);
    app.transcript_cells = vec![Arc::new(AgentMessageCell::new(
        vec![Line::from("Colored result.")],
        /*is_first_line*/ true,
    ))];
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let cwd = app.config.cwd.to_path_buf();
    app.handle_consolidate_agent_message(
        &mut tui,
        "Colored result.".into(),
        Some(MessagePhase::FinalAnswer),
        cwd,
        /*inline_visualization_context*/ None,
        ConsolidationScrollbackReflow::Required,
        /*deferred_history_cell*/ None,
    )?;
    assert_eq!(app.transcript_cells.len(), 1);
    let lines = app.transcript_cells[0].display_lines(/*width*/ 48);
    assert_eq!(
        lines[0].spans[0].style.fg,
        Some(history_cell::final_message_color(Some(color)))
    );
    assert_eq!(
        ratatui::text::Text::from(lines).to_string(),
        "┃ Colored result."
    );
    Ok(())
}
