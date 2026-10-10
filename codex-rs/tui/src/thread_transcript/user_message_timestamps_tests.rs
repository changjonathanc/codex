use super::*;
use crate::legacy_core::config::ConfigBuilder;
use codex_config::LoaderOverrides;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn user_message_timestamps_project_saved_times_in_item_order() -> anyhow::Result<()> {
    for enabled in [false, true] {
        let home = tempfile::tempdir()?;
        std::fs::write(
            home.path().join("config.toml"),
            format!("[tui]\nuser_message_timestamps = {enabled}\n"),
        )?;
        let config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
            .build()
            .await?;
        let item = |id: &str| ThreadItem::UserMessage {
            id: id.into(),
            client_id: None,
            content: vec![UserInput::Text {
                text: id.into(),
                text_elements: Vec::new(),
            }],
        };
        let times = HashMap::from([
            ("first".into(), 1_791_640_938_000),
            ("steer".into(), 1_791_641_202_000),
        ]);
        let cells = thread_items_to_transcript_cells_with_timestamps(
            /*thread_id*/ None,
            &config.cwd,
            [item("first"), item("unknown"), item("steer")],
            RawReasoningVisibility::Hidden,
            Some(&config),
            Some(&times),
        );
        let actual = cells
            .iter()
            .filter_map(|cell| cell.as_any().downcast_ref::<UserHistoryCell>())
            .map(|user| {
                (
                    user.message.clone(),
                    user.timestamp.map(|timestamp| timestamp.timestamp_millis()),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            vec![
                ("first".into(), enabled.then_some(1_791_640_938_000)),
                ("unknown".into(), None),
                ("steer".into(), enabled.then_some(1_791_641_202_000)),
            ]
        );
    }
    Ok(())
}
