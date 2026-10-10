use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn user_message_timestamps_can_be_disabled_for_live_input() {
    for enabled in [false, true] {
        let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
        while rx.try_recv().is_ok() {}
        chat.local_settings.tui.user_message_timestamps = enabled;
        chat.on_user_message_display_at(
            ChatWidget::user_message_display_from_inputs(&[UserInput::Text {
                text: "Timestamp test".into(),
                text_elements: Vec::new(),
            }]),
            Some(1_791_640_938_000),
        );
        let times = std::iter::from_fn(|| rx.try_recv().ok())
            .filter_map(|event| match event {
                AppEvent::InsertHistoryCell(cell) => cell
                    .as_any()
                    .downcast_ref::<UserHistoryCell>()
                    .map(|user| user.timestamp.map(|timestamp| timestamp.timestamp_millis())),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(times, vec![enabled.then_some(1_791_640_938_000)]);
    }
}

#[tokio::test]
async fn user_message_timestamps_replay_saved_item_times_and_skip_unknown_steers() {
    for enabled in [false, true] {
        let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
        while rx.try_recv().is_ok() {}
        chat.local_settings.tui.user_message_timestamps = enabled;
        chat.turn_lifecycle
            .user_message_started_at_ms
            .insert("saved-steer".into(), 1_791_641_202_000);
        let user = |id: &str| ThreadItem::UserMessage {
            id: id.into(),
            client_id: None,
            content: vec![UserInput::Text {
                text: id.into(),
                text_elements: Vec::new(),
            }],
        };
        chat.replay_thread_turns(
            vec![Turn {
                id: "timestamp-turn".into(),
                items: vec![user("first"), user("saved-steer"), user("unknown-steer")],
                items_view: codex_app_server_protocol::TurnItemsView::Full,
                status: TurnStatus::Completed,
                error: None,
                started_at: Some(1_791_640_938),
                completed_at: None,
                duration_ms: None,
            }],
            ReplayKind::ResumeInitialMessages,
        );
        let times = std::iter::from_fn(|| rx.try_recv().ok())
            .filter_map(|event| match event {
                AppEvent::InsertHistoryCell(cell) => cell
                    .as_any()
                    .downcast_ref::<UserHistoryCell>()
                    .map(|user| user.timestamp.map(|timestamp| timestamp.timestamp_millis())),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            times,
            vec![
                enabled.then_some(1_791_640_938_000),
                enabled.then_some(1_791_641_202_000),
                None
            ]
        );
        assert_eq!(chat.turn_lifecycle.replay_user_message_timestamp_ms, None);
    }
}
