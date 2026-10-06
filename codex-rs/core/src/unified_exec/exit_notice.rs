use tokio::sync::watch;

/// Resolves the race between initial output, later polls, and process exit.
#[derive(Clone, Copy)]
enum State {
    Pending,
    Armed,
    Reported,
}

pub(super) struct ExitNotice(watch::Sender<State>);

impl ExitNotice {
    pub(super) fn new() -> Self {
        Self(watch::channel(State::Pending).0)
    }

    pub(super) fn arm(&self) {
        self.0.send_replace(State::Armed);
    }

    pub(super) fn reported(&self) {
        self.0.send_replace(State::Reported);
    }

    pub(super) fn cancel_if_pending(&self) {
        self.0.send_if_modified(|state| {
            if matches!(state, State::Pending) {
                *state = State::Reported;
                true
            } else {
                false
            }
        });
    }

    pub(super) async fn should_notify(&self) -> bool {
        let mut state = self.0.subscribe();
        loop {
            match *state.borrow_and_update() {
                State::Armed => return true,
                State::Reported => return false,
                State::Pending => {}
            }
            if state.changed().await.is_err() {
                return false;
            }
        }
    }
}
