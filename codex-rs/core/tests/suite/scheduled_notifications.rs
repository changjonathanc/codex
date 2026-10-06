use anyhow::Result;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::EventMsg;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;

use super::unified_exec::submit_unified_exec_turn;

#[tokio::test]
async fn notify_on_exit_waits_for_the_busy_turn_then_starts_internal_turn() -> Result<()> {
    core_test_support::skip_if_target_windows!(Ok(()), "uses POSIX sleep");
    let server = start_mock_server().await;
    let mut builder = test_codex();
    let test = builder.build_with_auto_env(&server).await?;
    let log = mount_sse_sequence(&server, vec![
        sse(vec![ev_function_call("background", "exec_command", &json!({"cmd":"sleep 0.8; printf EXIT_PROOF", "yield_time_ms":250, "notify_on_exit":true}).to_string()), ev_completed("one")]),
        sse(vec![ev_function_call("foreground", "exec_command", &json!({"cmd":"sleep 1; printf BUSY_PROOF"}).to_string()), ev_completed("two")]),
        sse(vec![ev_assistant_message("waiting", "original turn finished"), ev_completed("three")]),
        sse(vec![ev_assistant_message("notified", "completion received"), ev_completed("four")]),
    ]).await;
    submit_unified_exec_turn(&test, "run", PermissionProfile::Disabled).await?;
    let first = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let second = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let (EventMsg::TurnComplete(first), EventMsg::TurnComplete(second)) = (first, second) else {
        unreachable!()
    };
    assert_ne!(first.turn_id, second.turn_id);
    let requests = log.requests();
    assert_eq!(requests.len(), 4);
    for request in &requests[..3] {
        assert!(
            !request.body_json()["input"]
                .to_string()
                .contains("scheduled_notification")
        );
    }
    let final_input = requests[3].body_json()["input"].to_string();
    assert!(final_input.contains("scheduled_notification"));
    assert!(final_input.contains("EXIT_PROOF"));
    assert!(final_input.contains("not new user authorization"));
    Ok(())
}

#[test_case("printf SYNC_PROOF", true, false; "synchronous_exit")]
#[test_case("sleep 0.6", true, true; "poll_reports_exit")]
#[test_case("sleep 0.6", false, false; "default_no_notice")]
#[tokio::test]
async fn notify_on_exit_does_not_duplicate_reported_or_disabled_completion(
    command: &str,
    notify: bool,
    poll: bool,
) -> Result<()> {
    core_test_support::skip_if_target_windows!(Ok(()), "uses POSIX sleep");
    let server = start_mock_server().await;
    let mut builder = test_codex();
    let test = builder.build_with_auto_env(&server).await?;
    let mut replies = vec![sse(vec![
        ev_function_call(
            "job",
            "exec_command",
            &json!({"cmd":command, "yield_time_ms":250, "notify_on_exit":notify}).to_string(),
        ),
        ev_completed("one"),
    ])];
    if poll {
        replies.push(sse(vec![
            ev_function_call(
                "poll",
                "write_stdin",
                &json!({"session_id":1000}).to_string(),
            ),
            ev_completed("two"),
        ]));
    }
    replies.push(sse(vec![
        ev_assistant_message("done", "done"),
        ev_completed("final"),
    ]));
    let log = mount_sse_sequence(&server, replies).await;
    submit_unified_exec_turn(&test, "run", PermissionProfile::Disabled).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    assert_eq!(log.requests().len(), if poll { 3 } else { 2 });
    Ok(())
}
