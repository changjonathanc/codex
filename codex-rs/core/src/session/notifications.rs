use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::TurnInput;
use super::session::Session;
use crate::context::ContextualUserFragment;
use crate::context::ScheduledNotification;

#[derive(Default)]
struct PendingNotifications(Mutex<NotificationState>);

#[derive(Default)]
struct NotificationState {
    closed: bool,
    items: BTreeMap<String, ScheduledNotification>,
}

impl Session {
    /// Queue internal work without steering the current turn. Repeated keys coalesce.
    pub(crate) async fn queue_notification(self: &Arc<Self>, notification: ScheduledNotification) {
        let pending = self
            .services
            .thread_extension_data
            .get_or_init::<PendingNotifications>(PendingNotifications::default);
        {
            let mut state = pending.0.lock().await;
            if state.closed || notification.is_cancelled() {
                return;
            }
            if state.items.len() >= 128 && !state.items.contains_key(&notification.id) {
                tracing::warn!("scheduled notification queue is full");
                return;
            }
            state.items.insert(notification.id.clone(), notification);
        }
        self.maybe_start_turn_for_pending_work().await;
    }

    pub(crate) async fn cancel_notification(&self, id: &str) {
        if let Some(pending) = self
            .services
            .thread_extension_data
            .get::<PendingNotifications>()
        {
            pending.0.lock().await.items.remove(id);
        }
    }

    pub(crate) async fn has_pending_notifications(&self) -> bool {
        if self.is_interrupted() {
            return false;
        }
        match self
            .services
            .thread_extension_data
            .get::<PendingNotifications>()
        {
            Some(pending) => pending
                .0
                .lock()
                .await
                .items
                .values()
                .any(|item| !item.is_cancelled()),
            None => false,
        }
    }

    pub(crate) async fn take_pending_notifications(&self) -> Vec<TurnInput> {
        let Some(pending) = self
            .services
            .thread_extension_data
            .get::<PendingNotifications>()
        else {
            return Vec::new();
        };
        std::mem::take(&mut pending.0.lock().await.items)
            .into_values()
            .filter(|item| !item.is_cancelled())
            .map(|item| TurnInput::ResponseItem(ContextualUserFragment::into(item).into()))
            .collect()
    }

    pub(crate) async fn close_notifications(&self) {
        let pending = self
            .services
            .thread_extension_data
            .get_or_init::<PendingNotifications>(PendingNotifications::default);
        let mut state = pending.0.lock().await;
        state.closed = true;
        state.items.clear();
    }
}
