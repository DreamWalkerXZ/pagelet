#[path = "mcp/support.rs"]
mod support;

use serde_json::json;
use support::TestServer;

#[tokio::test]
async fn reads_current_markdown_by_immutable_page_id() {
    let server = TestServer::start(true, None).await;
    let alice = server.user("alice").await;
    let bob = server.user("bob").await;
    let page = server.publish(&alice, "draft", "# Draft").await;
    let id = page["id"].as_i64().unwrap();
    server
        .client
        .patch(format!("{}/_/api/v1/pages/{id}", server.base_url))
        .bearer_auth(&alice)
        .json(&json!({"version": 1, "slug": "guide", "markdown": "# Guide\n\nUpdated."}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let owner = server.connect(&alice).await;
    let tools = owner.tools().await;
    let read = tools
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "read_page")
        .expect("read_page should be discoverable");
    assert_eq!(read["annotations"]["readOnlyHint"], true);
    assert_eq!(read["inputSchema"]["required"], json!(["id"]));

    let result = owner.call("read_page", json!({"id": id})).await;
    assert_eq!(
        result["structuredContent"],
        json!({"id": id, "slug": "guide", "markdown": "# Guide\n\nUpdated.", "version": 2})
    );

    let denied = server
        .connect(&bob)
        .await
        .call("read_page", json!({"id": id}))
        .await;
    assert_eq!(denied["isError"], true);
    assert_eq!(denied["structuredContent"], json!({"code": "forbidden"}));
}

#[tokio::test]
async fn applies_management_permissions_to_protected_markdown_without_revealing_secrets() {
    let server = TestServer::start(true, None).await;
    let alice = server.user("alice").await;
    let bob = server.user("bob").await;
    let page: serde_json::Value = server
        .client
        .post(format!("{}/_/api/v1/pages", server.base_url))
        .bearer_auth(&alice)
        .json(&json!({
            "slug": "protected",
            "markdown": "# Protected",
            "access_password": "test access password"
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let arguments = json!({"id": page["id"]});

    let unauthenticated = server
        .mcp_request(json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {"name": "read_page", "arguments": arguments}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), 401);

    let denied = server
        .connect(&bob)
        .await
        .call("read_page", arguments.clone())
        .await;
    assert_eq!(denied["isError"], true);
    assert_eq!(denied["structuredContent"], json!({"code": "forbidden"}));
    assert!(!denied.to_string().contains("# Protected"));

    let admin = server
        .connect(&server.admin)
        .await
        .call("read_page", arguments)
        .await;
    assert_eq!(
        admin["structuredContent"],
        json!({"id": page["id"], "slug": "protected", "markdown": "# Protected", "version": 1})
    );
    let response = admin.to_string();
    for secret in ["test access password", "access_password", "$argon2"] {
        assert!(!response.contains(secret));
    }
}
