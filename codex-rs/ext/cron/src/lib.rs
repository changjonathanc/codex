//! In-memory UTC schedules. Both cron and shell completion use Core's idle event queue.

use chrono::DateTime;
use chrono::Utc;
use codex_core::ThreadManager;
use codex_core::context::ScheduledNotification;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::FunctionCallError;
use codex_extension_api::JsonToolOutput;
use codex_extension_api::ResponsesApiTool;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ThreadStopInput;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolContributor;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolExecutorFuture;
use codex_extension_api::ToolName;
use codex_extension_api::ToolOutput;
use codex_extension_api::ToolSpec;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_tools::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Weak;
use tokio::sync::Mutex;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

mod schedule;
use schedule::Schedule;

struct CronExtension(Weak<ThreadManager>);

/// Install session-scoped schedules; no job is restored from disk.
pub fn install<C: Send + Sync + 'static>(
    builder: &mut ExtensionRegistryBuilder<C>,
    manager: Weak<ThreadManager>,
) {
    let extension = Arc::new(CronExtension(manager));
    builder.thread_lifecycle_contributor(extension.clone());
    builder.tool_contributor(extension);
}

impl<C: Send + Sync + 'static> ThreadLifecycleContributor<C> for CronExtension {
    fn on_thread_start<'a>(&'a self, input: ThreadStartInput<'a, C>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            if matches!(
                input.session_source,
                SessionSource::SubAgent(SubAgentSource::Review)
            ) {
                return;
            }
            let Ok(thread_id) = ThreadId::from_string(input.thread_store.level_id()) else {
                return;
            };
            let (changed, receiver) = watch::channel(());
            let runtime = input.thread_store.get_or_init::<Runtime>(|| Runtime {
                manager: self.0.clone(),
                thread_id,
                jobs: Mutex::new(BTreeMap::new()),
                changed,
                stopped: CancellationToken::new(),
            });
            tokio::spawn(run(
                Arc::downgrade(&runtime),
                receiver,
                runtime.stopped.clone(),
            ));
        })
    }

    fn on_thread_stop<'a>(&'a self, input: ThreadStopInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            if let Some(runtime) = input.thread_store.get::<Runtime>() {
                runtime.stopped.cancel();
            }
        })
    }
}

impl ToolContributor for CronExtension {
    fn tools(
        &self,
        _session: &ExtensionData,
        thread: &ExtensionData,
    ) -> Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>> {
        thread
            .get::<Runtime>()
            .map(|runtime| {
                vec![Arc::new(CronTool(runtime))
                    as Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>]
            })
            .unwrap_or_default()
    }
}

struct Runtime {
    manager: Weak<ThreadManager>,
    thread_id: ThreadId,
    jobs: Mutex<BTreeMap<String, Job>>,
    changed: watch::Sender<()>,
    stopped: CancellationToken,
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.stopped.cancel();
    }
}

struct Job {
    expression: String,
    schedule: Schedule,
    message: String,
    next: DateTime<Utc>,
    cancellation: CancellationToken,
}

async fn run(runtime: Weak<Runtime>, mut changed: watch::Receiver<()>, stopped: CancellationToken) {
    loop {
        let next = {
            let Some(runtime) = runtime.upgrade() else {
                return;
            };
            let now = Utc::now();
            let (due, next) = {
                let mut jobs = runtime.jobs.lock().await;
                let mut due = Vec::new();
                for (id, job) in &mut *jobs {
                    if job.next <= now {
                        due.push((
                            id.clone(),
                            format!("Cron job {id} fired (UTC): {}", job.message),
                            job.cancellation.clone(),
                        ));
                        let Some(next) = job.schedule.next(now) else {
                            continue;
                        };
                        job.next = next;
                    }
                }
                (due, jobs.values().map(|job| job.next).min())
            };
            if let Some(manager) = runtime.manager.upgrade()
                && let Ok(thread) = manager.get_thread(runtime.thread_id).await
            {
                for (id, message, cancellation) in due {
                    if let Ok(notification) =
                        ScheduledNotification::new(format!("cron:{id}"), message, cancellation)
                    {
                        thread.queue_notification(notification).await;
                    }
                }
            }
            next
        };
        tokio::select! {
            _ = stopped.cancelled() => return,
            result = changed.changed() => if result.is_err() { return; },
            _ = async {
                match next {
                    Some(next) => tokio::time::sleep((next - Utc::now()).to_std().unwrap_or_default()).await,
                    None => std::future::pending().await,
                }
            } => {},
        }
    }
}

struct CronTool(Arc<Runtime>);

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
enum Args {
    Create { schedule: String, message: String },
    List,
    Delete { id: String },
}

impl<'call> ToolExecutor<ToolCall<'call>> for CronTool {
    fn tool_name(&self) -> ToolName {
        ToolName::plain("cron")
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec::Function(ResponsesApiTool {
            name: "cron".into(),
            description: "Create, list, or delete recurring five-field UTC cron jobs. Fields: minute hour day-of-month month day-of-week; supports *, numbers, lists, ranges and steps (Sunday 0 or 7). Example: */5 * * * *. Each firing sends the message as internal input AFTER the current turn, or starts an idle turn. Missed fires coalesce. At most 16 jobs; messages at most 400 bytes. Jobs exist only while this thread is loaded; no restart recovery. Delete jobs when no longer needed.".into(),
            strict: false, defer_loading: None, output_schema: None,
            parameters: JsonSchema::object(BTreeMap::from([
                ("action".into(), JsonSchema::string_enum(vec![json!("create"), json!("list"), json!("delete")], /*description*/ None)),
                ("schedule".into(), JsonSchema::string(Some("Required for create. Five-field cron in UTC.".into()))),
                ("message".into(), JsonSchema::string(Some("Required for create. Work to perform when this job fires.".into()))),
                ("id".into(), JsonSchema::string(Some("Required for delete. Job ID returned by create or list.".into()))),
            ]), Some(vec!["action".into()]), Some(false.into())),
        })
    }

    fn handle<'a>(&'a self, call: ToolCall<'call>) -> ToolExecutorFuture<'a>
    where
        'call: 'a,
    {
        Box::pin(async move {
            let args: Args = serde_json::from_str(call.function_arguments()?)
                .map_err(|error| FunctionCallError::RespondToModel(error.to_string()))?;
            let (result, deleted_id) = {
                let mut jobs = self.0.jobs.lock().await;
                match args {
                    Args::Create { schedule, message } => {
                        if jobs.len() >= 16 || message.is_empty() || message.len() > 400 {
                            return Err(FunctionCallError::RespondToModel(
                                "At most 16 jobs; use a nonempty message of at most 400 bytes."
                                    .into(),
                            ));
                        }
                        let parsed = Schedule::parse(&schedule)
                            .map_err(FunctionCallError::RespondToModel)?;
                        let next = parsed.next(Utc::now()).ok_or_else(|| {
                            FunctionCallError::RespondToModel(
                                "Schedule has no possible firing in the next eight years.".into(),
                            )
                        })?;
                        let id = uuid::Uuid::new_v4().to_string();
                        let cancellation = CancellationToken::new();
                        ScheduledNotification::new(
                            format!("cron:{id}"),
                            format!("Cron job {id} fired (UTC): {message}"),
                            cancellation.clone(),
                        )
                        .map_err(FunctionCallError::RespondToModel)?;
                        jobs.insert(
                            id.clone(),
                            Job {
                                expression: schedule.clone(),
                                schedule: parsed,
                                message,
                                next,
                                cancellation,
                            },
                        );
                        (
                            json!({"id": id, "schedule": schedule, "timezone": "UTC", "nextFire": next}),
                            None,
                        )
                    }
                    Args::List => (
                        json!({"jobs": jobs.iter().map(|(id, job)| json!({"id": id, "schedule": job.expression, "message": job.message, "timezone": "UTC", "nextFire": job.next})).collect::<Vec<_>>()}),
                        None,
                    ),
                    Args::Delete { id } => {
                        let deleted = if let Some(job) = jobs.remove(&id) {
                            job.cancellation.cancel();
                            true
                        } else {
                            false
                        };
                        (json!({"deleted": deleted}), Some(id))
                    }
                }
            };
            if let Some(id) = deleted_id
                && let Some(manager) = self.0.manager.upgrade()
                && let Ok(thread) = manager.get_thread(self.0.thread_id).await
            {
                thread.cancel_notification(&format!("cron:{id}")).await;
            }
            self.0.changed.send_replace(());
            Ok(Box::new(JsonToolOutput::new(result)) as Box<dyn ToolOutput>)
        })
    }
}
