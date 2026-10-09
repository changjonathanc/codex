use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn final_message_phase_survives_live_completion_and_replay() {
    for from_replay in [false, true] {
        for phase in [
            None,
            Some(MessagePhase::Commentary),
            Some(MessagePhase::FinalAnswer),
        ] {
            let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
            while rx.try_recv().is_ok() {}
            chat.on_agent_message_item_completed(
                AgentMessageItem {
                    id: "phase-review".into(),
                    content: vec![AgentMessageContent::Text {
                        text: "The final result.".into(),
                    }],
                    phase: phase.clone(),
                    memory_citation: None,
                    delivery: None,
                    questions: None,
                },
                "turn-review",
                from_replay,
            );
            let mut completed = Vec::new();
            while let Ok(event) = rx.try_recv() {
                match event {
                    AppEvent::ConsolidateAgentMessage {
                        source,
                        phase: actual,
                        cwd,
                        inline_visualization_context,
                        scrollback_reflow,
                        ..
                    } => {
                        assert_eq!(actual, phase);
                        if actual == Some(MessagePhase::FinalAnswer) {
                            assert_eq!(
                                scrollback_reflow,
                                crate::app_event::ConsolidationScrollbackReflow::Required
                            );
                        }
                        completed.push(
                            history_cell::AgentMarkdownCell::new_with_inline_visualizations(
                                source,
                                &cwd,
                                inline_visualization_context,
                            )
                            .with_message_phase(actual)
                            .display_lines(/*width*/ 48),
                        );
                    }
                    AppEvent::InsertHistoryCell(cell) if from_replay => {
                        completed.push(cell.display_lines(/*width*/ 48))
                    }
                    _ => {}
                }
            }
            assert_eq!(completed.len(), 1);
            let text = lines_to_single_string(&completed[0]);
            assert!(text.contains("The final result."));
            assert_eq!(
                text.contains("FINAL ANSWER"),
                phase == Some(MessagePhase::FinalAnswer)
            );
        }
    }
}
