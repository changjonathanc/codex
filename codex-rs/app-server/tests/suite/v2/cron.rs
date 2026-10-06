use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput;
use core_test_support::responses;
use serde_json::json;
use tempfile::TempDir;

#[tokio::test]
async fn cron_agent_can_create_list_and_delete_jobs() -> Result<()> {
    let server = responses::start_mock_server().await;
    let log = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("one"),
                responses::ev_function_call(
                    "create",
                    "cron",
                    &json!({"action":"create", "schedule":"0 9 * * 1-5", "message":"check CI"})
                        .to_string(),
                ),
                responses::ev_completed("one"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("two"),
                responses::ev_function_call("list", "cron", &json!({"action":"list"}).to_string()),
                responses::ev_completed("two"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("done", "created"),
                responses::ev_completed("three"),
            ]),
        ],
    )
    .await;
    let home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_provider_config("supports_websockets = false")
        .write(home.path())?;
    let mut app = TestAppServer::builder()
        .with_codex_home(home.path())
        .without_managed_config()
        .build_initialized()
        .await?;
    let thread = app.start_thread(ThreadStartParams::default()).await?.thread;
    app.start_turn_and_wait_for_completion(TurnStartParams {
        thread_id: thread.id.clone(),
        input: vec![UserInput::Text {
            text: "create a cron job".into(),
            text_elements: Vec::new(),
        }],
        ..Default::default()
    })
    .await?;
    let requests = log.requests();
    let output = requests
        .last()
        .unwrap()
        .function_call_output_content_and_success("list")
        .unwrap()
        .0
        .unwrap();
    let list: serde_json::Value = serde_json::from_str(&output)?;
    let id = list["jobs"][0]["id"].as_str().unwrap();
    let deleted_log = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_function_call(
                    "delete",
                    "cron",
                    &json!({"action":"delete", "id":id}).to_string(),
                ),
                responses::ev_completed("four"),
            ]),
            responses::sse(vec![
                responses::ev_function_call("empty", "cron", &json!({"action":"list"}).to_string()),
                responses::ev_completed("five"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("end", "deleted"),
                responses::ev_completed("six"),
            ]),
        ],
    )
    .await;
    app.start_turn_and_wait_for_completion(TurnStartParams {
        thread_id: thread.id,
        input: vec![UserInput::Text {
            text: "delete it".into(),
            text_elements: Vec::new(),
        }],
        ..Default::default()
    })
    .await?;
    let requests = deleted_log.requests();
    for (call_id, expected) in [
        ("delete", json!({"deleted":true})),
        ("empty", json!({"jobs":[]})),
    ] {
        let output = requests
            .last()
            .unwrap()
            .function_call_output_content_and_success(call_id)
            .unwrap()
            .0
            .unwrap();
        pretty_assertions::assert_eq!(
            serde_json::from_str::<serde_json::Value>(&output)?,
            expected
        );
    }
    Ok(())
}
