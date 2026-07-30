//! Data model for grouped exec-call history cells in the TUI transcript.
//!
//! An `ExecCell` can represent a single command, an "exploring" group of related read/list/search
//! commands, or a group of agent commands whose lifetimes overlap. The chat widget relies on stable
//! `call_id` matching to route progress and end events into the right cell, and it treats "call id
//! not found" as a real signal (for example, an orphan end that should render as a separate history
//! entry).

use std::borrow::Cow;
use std::time::Duration;
use std::time::Instant;

use super::live_output::LiveCommandOutput;
use codex_app_server_protocol::CommandExecutionSource as ExecCommandSource;
use codex_protocol::parse_command::ParsedCommand;
use itertools::Either;

#[derive(Debug, Default)]
pub(crate) struct CommandOutput {
    pub(crate) exit_code: i32,
    /// The finalized, interleaved stderr and stdout that replaces any streamed preview.
    aggregated_output: String,
    /// The live preview while command-output deltas are still arriving.
    live_output: Option<LiveCommandOutput>,
}

impl CommandOutput {
    pub(crate) fn new(exit_code: i32, aggregated_output: String) -> Self {
        Self {
            exit_code,
            aggregated_output,
            live_output: None,
        }
    }

    /// Returns the total number of logical lines and the number retained for rendering.
    pub(super) fn line_counts(&self) -> (usize, usize) {
        match self.live_output.as_ref() {
            Some(output) => (output.total_lines(), output.retained_lines()),
            None => {
                let total = self.aggregated_output.lines().count();
                (total, total)
            }
        }
    }

    /// Returns retained preview lines with reverse traversal for efficient tail rendering.
    pub(super) fn lines(&self) -> impl DoubleEndedIterator<Item = Cow<'_, str>> {
        match self.live_output.as_ref() {
            Some(output) => Either::Left(output.lines()),
            None => Either::Right(self.aggregated_output.lines().map(Cow::Borrowed)),
        }
    }

    /// Returns lines for the expanded transcript, including any storage-level omission marker.
    pub(super) fn transcript_lines(&self) -> impl Iterator<Item = Cow<'_, str>> {
        match self.live_output.as_ref() {
            Some(output) => Either::Left(output.transcript_lines()),
            None => Either::Right(self.aggregated_output.lines().map(Cow::Borrowed)),
        }
    }
}

#[derive(Debug)]
pub(crate) struct ExecCall {
    pub(crate) call_id: String,
    pub(crate) command: Vec<String>,
    pub(crate) parsed: Vec<ParsedCommand>,
    pub(crate) output: Option<CommandOutput>,
    pub(crate) source: ExecCommandSource,
    /// Local start instant, retained after completion so parallel groups can report wall time.
    pub(crate) start_time: Option<Instant>,
    pub(crate) timeout: Option<Duration>,
    pub(crate) duration: Option<Duration>,
    pub(crate) interaction_input: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExecCellKind {
    Command,
    Exploring,
    Parallel,
}

#[derive(Debug)]
pub(crate) struct ExecCell {
    pub(crate) calls: Vec<ExecCall>,
    kind: ExecCellKind,
    animations_enabled: bool,
}

impl ExecCell {
    pub(crate) fn new(call: ExecCall, animations_enabled: bool) -> Self {
        let kind = if Self::is_exploring_call(&call) {
            ExecCellKind::Exploring
        } else {
            ExecCellKind::Command
        };
        Self {
            calls: vec![call],
            kind,
            animations_enabled,
        }
    }

    pub(crate) fn add_call(
        &mut self,
        call_id: String,
        command: Vec<String>,
        parsed: Vec<ParsedCommand>,
        source: ExecCommandSource,
        timeout: Option<Duration>,
        interaction_input: Option<String>,
    ) -> bool {
        let call = ExecCall {
            call_id,
            command,
            parsed,
            output: None,
            source,
            start_time: Some(Instant::now()),
            timeout,
            duration: None,
            interaction_input,
        };
        match self.kind {
            ExecCellKind::Exploring if Self::is_exploring_call(&call) => {
                self.calls.push(call);
                true
            }
            ExecCellKind::Command | ExecCellKind::Parallel
                if self.is_active() && self.can_group_parallel_call(&call) =>
            {
                self.kind = ExecCellKind::Parallel;
                self.calls.push(call);
                true
            }
            ExecCellKind::Command | ExecCellKind::Exploring | ExecCellKind::Parallel => false,
        }
    }

    /// Marks the most recently matching call as finished and returns whether a call was found.
    ///
    /// Callers should treat `false` as a routing mismatch rather than silently ignoring it. The
    /// chat widget uses that signal to avoid attaching an orphan `exec_end` event to an unrelated
    /// active exploring cell, which would incorrectly collapse two transcript entries together.
    pub(crate) fn complete_call(
        &mut self,
        call_id: &str,
        output: CommandOutput,
        duration: Duration,
    ) -> bool {
        let Some(call) = self.calls.iter_mut().rev().find(|c| c.call_id == call_id) else {
            return false;
        };
        call.output = Some(output);
        call.duration = Some(duration);
        true
    }

    pub(crate) fn should_flush(&self) -> bool {
        !self.is_exploring_cell() && !self.is_active()
    }

    pub(crate) fn mark_failed(&mut self) {
        for call in self.calls.iter_mut() {
            if call.duration.is_none() {
                let elapsed = call
                    .start_time
                    .map(|st| st.elapsed())
                    .unwrap_or_else(|| Duration::from_millis(0));
                call.duration = Some(elapsed);
                call.output
                    .get_or_insert_with(CommandOutput::default)
                    .exit_code = 1;
            }
        }
    }

    pub(crate) fn is_exploring_cell(&self) -> bool {
        self.kind == ExecCellKind::Exploring
    }

    pub(crate) fn is_parallel_cell(&self) -> bool {
        self.kind == ExecCellKind::Parallel
    }

    pub(crate) fn is_active(&self) -> bool {
        self.calls.iter().any(|c| c.duration.is_none())
    }

    pub(crate) fn active_start_time(&self) -> Option<Instant> {
        self.calls
            .iter()
            .find(|c| c.duration.is_none())
            .and_then(|c| c.start_time)
    }

    pub(crate) fn first_start_time(&self) -> Option<Instant> {
        self.calls.iter().filter_map(|call| call.start_time).min()
    }

    /// Returns first-start-to-last-finish time for a completed parallel group.
    pub(crate) fn parallel_duration(&self) -> Option<Duration> {
        if !self.is_parallel_cell() || self.is_active() {
            return None;
        }

        let longest_call = self.calls.iter().filter_map(|call| call.duration).max()?;
        let Some(first_start) = self.first_start_time() else {
            return Some(longest_call);
        };
        let measured_span = self
            .calls
            .iter()
            .filter_map(|call| {
                Some(call.start_time?.saturating_duration_since(first_start) + call.duration?)
            })
            .max();

        Some(measured_span.map_or(longest_call, |span| span.max(longest_call)))
    }

    pub(crate) fn animations_enabled(&self) -> bool {
        self.animations_enabled
    }

    pub(crate) fn iter_calls(&self) -> impl Iterator<Item = &ExecCall> {
        self.calls.iter()
    }

    pub(crate) fn append_output(&mut self, call_id: &str, chunk: &str) -> bool {
        if chunk.is_empty() {
            return false;
        }
        let Some(call) = self.calls.iter_mut().rev().find(|c| c.call_id == call_id) else {
            return false;
        };
        let output = call.output.get_or_insert_with(CommandOutput::default);
        output
            .live_output
            .get_or_insert_with(LiveCommandOutput::default)
            .push_str(chunk);
        true
    }

    pub(super) fn is_exploring_call(call: &ExecCall) -> bool {
        !matches!(call.source, ExecCommandSource::UserShell)
            && !call.parsed.is_empty()
            && call.parsed.iter().all(|p| {
                matches!(
                    p,
                    ParsedCommand::Read { .. }
                        | ParsedCommand::ListFiles { .. }
                        | ParsedCommand::Search { .. }
                )
            })
    }

    fn can_group_parallel_call(&self, call: &ExecCall) -> bool {
        matches!(call.source, ExecCommandSource::Agent)
            && self
                .calls
                .iter()
                .all(|existing| matches!(existing.source, ExecCommandSource::Agent))
    }
}

impl ExecCall {
    pub(crate) fn is_user_shell_command(&self) -> bool {
        matches!(self.source, ExecCommandSource::UserShell)
    }

    pub(crate) fn is_unified_exec_interaction(&self) -> bool {
        matches!(self.source, ExecCommandSource::UnifiedExecInteraction)
    }
}
