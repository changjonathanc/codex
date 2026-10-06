use anyhow::Result;
use codex_core::context::ScheduledNotification;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::EventMsg;
use core_test_support::responses;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn cron_notifications_coalesce_and_cancel_while_busy() -> Result<()> {
    core_test_support::skip_if_target_windows!(Ok(()), "uses POSIX sleep");
    let server = responses::start_mock_server().await;
    let mut builder = test_codex();
    let test = builder.build_with_auto_env(&server).await?;
    let log = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_function_call(
                    "busy",
                    "exec_command",
                    &json!({"cmd":"sleep 1"}).to_string(),
                ),
                responses::ev_completed("one"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("done", "original done"),
                responses::ev_completed("two"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("event", "event received"),
                responses::ev_completed("three"),
            ]),
        ],
    )
    .await;
    super::unified_exec::submit_unified_exec_turn(&test, "run", PermissionProfile::Disabled)
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::ExecCommandBegin(_))
    })
    .await;
    for (id, message) in [
        ("cron:repeat", "STALE_TICK"),
        ("cron:repeat", "LATEST_TICK"),
        ("cron:cancel", "CANCELLED_TICK"),
    ] {
        test.codex
            .queue_notification(
                ScheduledNotification::new(id.into(), message.into(), CancellationToken::new())
                    .unwrap(),
            )
            .await;
    }
    test.codex.cancel_notification("cron:cancel").await;
    // A producer may finish sending an event after its job has been deleted.
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    test.codex
        .queue_notification(
            ScheduledNotification::new(
                "cron:cancel".into(),
                "LATE_CANCELLED_TICK".into(),
                cancellation,
            )
            .unwrap(),
        )
        .await;
    for _ in 0..2 {
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
    }
    let requests = log.requests();
    assert_eq!(requests.len(), 3);
    let input = requests[2].body_json()["input"].to_string();
    assert!(input.contains("LATEST_TICK"));
    assert!(!input.contains("STALE_TICK"));
    assert!(!input.contains("CANCELLED_TICK"));
    Ok(())
}
