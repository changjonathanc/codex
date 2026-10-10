//! Restore message times from item-page metadata without changing stored prompt text.

use super::*;
use std::collections::HashMap;

pub(crate) fn thread_items_to_transcript_cells_with_timestamps(
    thread_id: Option<ThreadId>,
    cwd: &AbsolutePathBuf,
    items: impl IntoIterator<Item = ThreadItem>,
    visibility: RawReasoningVisibility,
    config: Option<&Config>,
    timestamps: Option<&HashMap<String, i64>>,
) -> TranscriptCells {
    let enabled = config.is_some_and(|config| {
        config
            .config_layer_stack
            .effective_config()
            .get("tui")
            .and_then(|tui| tui.get("user_message_timestamps"))
            .and_then(toml::Value::as_bool)
            .unwrap_or_default()
    });
    if !enabled {
        return thread_items_to_transcript_cells(thread_id, cwd, items, visibility, config);
    }
    let items = items.into_iter().collect::<Vec<_>>();
    let mut user_times = items.iter().filter_map(|item| match item {
        ThreadItem::UserMessage { id, .. } => {
            Some(timestamps.and_then(|times| times.get(id)).copied())
        }
        _ => None,
    });
    let mut cells =
        thread_items_to_transcript_cells(thread_id, cwd, items.iter().cloned(), visibility, config);
    for cell in &mut cells {
        if let Some(user) =
            Arc::get_mut(cell).and_then(|cell| cell.as_any_mut().downcast_mut::<UserHistoryCell>())
        {
            let timestamp = user_times.next().flatten();
            user.timestamp = timestamp
                .filter(|timestamp| *timestamp > 0)
                .and_then(chrono::DateTime::from_timestamp_millis)
                .map(|timestamp| timestamp.with_timezone(&chrono::Local));
        }
    }
    cells
}

#[cfg(test)]
#[path = "user_message_timestamps_tests.rs"]
mod tests;
