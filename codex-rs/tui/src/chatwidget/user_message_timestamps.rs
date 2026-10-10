//! Timestamp presentation for user input, separate from prompt content.

use super::*;

impl ChatWidget {
    pub(super) fn on_user_message_display(&mut self, display: UserMessageDisplay) {
        self.on_user_message_display_at(
            display,
            self.turn_lifecycle
                .replay_user_message_timestamp_ms
                .or_else(|| Some(chrono::Utc::now().timestamp_millis())),
        );
    }

    pub(super) fn on_user_message_display_at(
        &mut self,
        display: UserMessageDisplay,
        timestamp_ms: Option<i64>,
    ) {
        self.transcript.last_status_copy_targets = None;
        self.last_rendered_user_message_display = Some(display.clone());
        self.last_rendered_user_message_client_id = None;
        if !display.message.trim().is_empty()
            || !display.text_elements.is_empty()
            || !display.local_images.is_empty()
            || !display.remote_image_urls.is_empty()
        {
            let timestamp_ms = self
                .local_settings
                .tui
                .user_message_timestamps
                .then_some(timestamp_ms)
                .flatten();
            self.add_to_history(
                history_cell::new_user_prompt(
                    display.message,
                    display.text_elements,
                    display.local_images,
                    display.remote_image_urls,
                )
                .with_timestamp(timestamp_ms),
            );
        }
    }
}
