//! Progress and bounded output previews for overlapping agent shell commands.

use super::model::ExecCall;
use super::model::ExecCell;
use super::render::OutputLinesParams;
use super::render::TOOL_CALL_MAX_LINES;
use super::render::activity_marker;
use super::render::output_lines;
use crate::exec_command::strip_bash_lc_and_escape;
use crate::render::highlight::highlight_bash_to_lines;
use crate::render::line_utils::prefix_lines;
use crate::render::line_utils::push_owned_lines;
use crate::terminal_hyperlinks::plain_hyperlink_lines;
use crate::terminal_hyperlinks::visible_lines;
use crate::wrapping::RtOptions;
use crate::wrapping::adaptive_wrap_line;
use codex_utils_elapsed::format_duration;
use ratatui::prelude::*;
use textwrap::WordSplitter;
use unicode_width::UnicodeWidthStr;

const PARALLEL_OUTPUT_MAX_LINES: usize = 3;

impl ExecCell {
    pub(super) fn parallel_display_lines(&self, width: u16) -> Vec<Line<'static>> {
        debug_assert!(self.is_parallel_cell());
        let total = self.group.calls.len();
        let completed = self
            .group
            .calls
            .iter()
            .filter(|call| call.duration.is_some())
            .count();
        let is_active = self.is_active();
        let any_failed = self
            .group
            .calls
            .iter()
            .filter_map(|call| call.output.as_ref())
            .any(|output| output.exit_code != 0);
        let bullet = if is_active {
            activity_marker(self.first_start_time(), self.animations_enabled())
        } else if any_failed {
            "•".red().bold()
        } else {
            "•".green().bold()
        };
        let title = if is_active {
            format!("Running {total} commands in parallel")
        } else {
            format!("Ran {total} commands in parallel")
        };
        let mut header = Line::from(vec![bullet, " ".into(), title.bold()]);
        if is_active && completed > 0 {
            header.extend([
                " (".dim(),
                format!("{completed}/{total} done").dim(),
                ")".dim(),
            ]);
        } else if let Some(duration) = self.parallel_duration() {
            header.extend([
                " (".dim(),
                format!("{} wall", format_duration(duration)).dim(),
                ")".dim(),
            ]);
        }

        let mut lines = vec![header];
        for (index, call) in self.group.calls.iter().enumerate() {
            lines.extend(self.parallel_call_display_lines(call, index + 1 == total, width));
        }
        lines
    }

    fn parallel_call_display_lines(
        &self,
        call: &ExecCall,
        is_last: bool,
        width: u16,
    ) -> Vec<Line<'static>> {
        let branch_prefix = if is_last { "  └─ " } else { "  ├─ " };
        let continuation_tree = if is_last { "     " } else { "  │  " };
        let success = call
            .duration
            .and_then(|_| call.output.as_ref().map(|output| output.exit_code == 0));
        let marker = match success {
            Some(true) => "✓".green().bold(),
            Some(false) => "✗".red().bold(),
            None => activity_marker(call.start_time, self.animations_enabled()),
        };
        let timing = match call.duration {
            Some(duration) => Some(format_duration(duration)),
            None => call
                .timeout
                .map(|timeout| format!("timeout {}", format_duration(timeout))),
        };
        let mut command_line = Line::from(vec![branch_prefix.dim(), marker, " ".into()]);
        if let Some(timing) = timing {
            command_line.extend(["(".dim(), timing.dim(), ") ".dim()]);
        }
        let command_prefix_width = command_line.width();
        let branch_width = UnicodeWidthStr::width(branch_prefix);
        let continuation_prefix = format!(
            "{continuation_tree}{}",
            " ".repeat(command_prefix_width.saturating_sub(branch_width))
        );
        let command = strip_bash_lc_and_escape(&call.command);
        let highlighted_lines = highlight_bash_to_lines(&command);
        let continuation_wrap_width = usize::from(width)
            .saturating_sub(UnicodeWidthStr::width(continuation_prefix.as_str()))
            .max(/*other*/ 1);
        let continuation_options =
            RtOptions::new(continuation_wrap_width).word_splitter(WordSplitter::NoHyphenation);
        let mut continuation_lines = Vec::new();

        if let Some((first, rest)) = highlighted_lines.split_first() {
            let first_width = usize::from(width)
                .saturating_sub(command_prefix_width)
                .max(/*other*/ 1);
            let first_options =
                RtOptions::new(first_width).word_splitter(WordSplitter::NoHyphenation);
            let mut first_wrapped = Vec::new();
            push_owned_lines(
                &adaptive_wrap_line(first, first_options),
                &mut first_wrapped,
            );
            let mut first_wrapped = first_wrapped.into_iter();
            if let Some(first_segment) = first_wrapped.next() {
                command_line.extend(first_segment);
            }
            continuation_lines.extend(first_wrapped);
            for line in rest {
                push_owned_lines(
                    &adaptive_wrap_line(line, continuation_options.clone()),
                    &mut continuation_lines,
                );
            }
        }

        let mut lines = vec![command_line];
        let continuation_lines = Self::limit_lines_from_start(
            &plain_hyperlink_lines(continuation_lines),
            /*keep*/ 2,
        );
        if !continuation_lines.is_empty() {
            lines.extend(prefix_lines(
                visible_lines(continuation_lines),
                continuation_prefix.clone().dim(),
                continuation_prefix.dim(),
            ));
        }

        let Some(output) = call.output.as_ref() else {
            return lines;
        };
        let raw_output = output_lines(
            Some(output),
            OutputLinesParams {
                line_limit: TOOL_CALL_MAX_LINES,
                only_err: false,
                include_angle_pipe: false,
                include_prefix: false,
            },
        );
        if raw_output.lines.is_empty() {
            return lines;
        }

        let output_initial_prefix = format!("{continuation_tree}└─ ");
        let output_subsequent_prefix = format!("{continuation_tree}   ");
        let output_wrap_width = usize::from(width)
            .saturating_sub(
                UnicodeWidthStr::width(output_initial_prefix.as_str())
                    .max(UnicodeWidthStr::width(output_subsequent_prefix.as_str())),
            )
            .max(/*other*/ 1);
        let output_options =
            RtOptions::new(output_wrap_width).word_splitter(WordSplitter::NoHyphenation);
        let mut wrapped_output = Vec::new();
        for line in &raw_output.lines {
            push_owned_lines(
                &adaptive_wrap_line(line, output_options.clone()),
                &mut wrapped_output,
            );
        }
        let prefixed_output = prefix_lines(
            wrapped_output,
            output_initial_prefix.dim(),
            output_subsequent_prefix.clone().into(),
        );
        lines.extend(visible_lines(Self::truncate_lines_middle(
            &plain_hyperlink_lines(prefixed_output),
            PARALLEL_OUTPUT_MAX_LINES,
            width,
            raw_output.omitted,
            Some(Line::from(output_subsequent_prefix.dim())),
        )));
        lines
    }
}
