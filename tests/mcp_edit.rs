#[allow(dead_code)]
#[path = "mcp/support.rs"]
mod support;

use serde_json::{Value, json};
use support::TestServer;

#[tokio::test]
async fn edits_markdown_and_slug_by_page_id_and_version() {
    let server = TestServer::start(true, Some("https://pages.example:8443/")).await;
    let alice = server.user("alice").await;
    let page = server.publish(&alice, "draft", "# Draft").await;
    let id = &page["id"];
    let client = server.connect(&alice).await;
    let tools = client.tools().await;
    let edit = tools
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "edit_page")
        .expect("edit_page should be discoverable");
    assert_eq!(edit["annotations"]["readOnlyHint"], false);
    assert_eq!(edit["inputSchema"]["required"], json!(["id", "version"]));
    assert_eq!(edit["inputSchema"]["additionalProperties"], false);

    let read = client.call("read_page", json!({"id": id})).await;
    let updated = client
        .call(
            "edit_page",
            json!({"id": id, "version": read["structuredContent"]["version"], "markdown": "# Guide"}),
        )
        .await;
    assert_eq!(
        updated["structuredContent"],
        json!({"id": id, "slug": "draft", "version": 2, "url": "https://pages.example:8443/draft"})
    );

    let renamed = client
        .call(
            "edit_page",
            json!({"id": id, "version": updated["structuredContent"]["version"], "slug": "guide"}),
        )
        .await;
    assert_eq!(
        renamed["structuredContent"],
        json!({"id": id, "slug": "guide", "version": 3, "url": "https://pages.example:8443/guide"})
    );
    let current = client.call("read_page", json!({"id": id})).await;
    assert_eq!(
        current["structuredContent"],
        json!({"id": id, "slug": "guide", "version": 3, "markdown": "# Guide"})
    );
}

#[tokio::test]
async fn edit_conflicts_preserve_the_current_page_and_explain_stale_recovery() {
    let server = TestServer::start(true, None).await;
    let alice = server.user("alice").await;
    let page = server.publish(&alice, "draft", "# Draft").await;
    server
        .publish(&server.admin, "occupied", "# Occupied")
        .await;
    let id = &page["id"];
    let client = server.connect(&alice).await;
    let read = client.call("read_page", json!({"id": id})).await;
    let version = &read["structuredContent"]["version"];

    let taken = client
        .call(
            "edit_page",
            json!({"id": id, "version": version, "slug": "occupied", "markdown": "# Replaced"}),
        )
        .await;
    assert_eq!(taken["isError"], true);
    assert_eq!(taken["structuredContent"], json!({"code": "slug_taken"}));
    assert_eq!(
        client.call("read_page", json!({"id": id})).await["structuredContent"],
        read["structuredContent"]
    );

    server
        .client
        .patch(format!("{}/_/api/v1/pages/{id}", server.base_url))
        .bearer_auth(&alice)
        .json(&json!({"version": version, "slug": "current", "markdown": "# Current"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let stale = client
        .call(
            "edit_page",
            json!({"id": id, "version": version, "slug": "older", "markdown": "# Outdated"}),
        )
        .await;
    assert_eq!(stale["isError"], true);
    assert_eq!(stale["structuredContent"]["code"], "stale_version");
    let message = stale["structuredContent"]["message"].as_str().unwrap();
    assert!(message.contains("read_page") && message.contains("reconcile"));
    let current = client.call("read_page", json!({"id": id})).await;
    assert_eq!(
        current["structuredContent"],
        json!({"id": id, "slug": "current", "version": 2, "markdown": "# Current"})
    );
}

#[tokio::test]
async fn protected_page_edits_require_management_access_and_preserve_the_access_password() {
    let server = TestServer::start(true, None).await;
    let alice = server.user("alice").await;
    let bob = server.user("bob").await;
    let page: Value = server
        .client
        .post(format!("{}/_/api/v1/pages", server.base_url))
        .bearer_auth(&alice)
        .json(&json!({
            "slug": "private",
            "markdown": "# Private",
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
    let id = &page["id"];
    let arguments = json!({"id": id, "version": 1, "slug": "renamed", "markdown": "# Updated"});

    let denied = server
        .connect(&bob)
        .await
        .call("edit_page", arguments.clone())
        .await;
    assert_eq!(denied["isError"], true);
    assert_eq!(denied["structuredContent"], json!({"code": "forbidden"}));

    let approved = server
        .connect(&server.admin)
        .await
        .call("edit_page", arguments)
        .await;
    assert_eq!(
        approved["structuredContent"],
        json!({"id": id, "slug": "renamed", "version": 2, "url": "/renamed"})
    );

    let raw = format!("{}/_/raw/renamed", server.base_url);
    assert_eq!(server.client.get(&raw).send().await.unwrap().status(), 401);
    let grant: Value = server
        .client
        .post(format!(
            "{}/_/api/v1/pages/by-slug/renamed/verify-password",
            server.base_url
        ))
        .json(&json!({"password": "test access password"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        server
            .client
            .get(&raw)
            .header("X-Pagelet-Grant", grant["grant"].as_str().unwrap())
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
        "# Updated"
    );
    let owner = server.connect(&alice).await;
    let current = owner.call("read_page", json!({"id": id})).await;
    assert_eq!(current["structuredContent"]["markdown"], "# Updated");
}

#[tokio::test]
async fn requires_an_edit_field_and_accepts_empty_or_whitespace_markdown() {
    let server = TestServer::start(true, None).await;
    let alice = server.user("alice").await;
    let page = server.publish(&alice, "draft", "# Draft").await;
    let id = &page["id"];
    let client = server.connect(&alice).await;

    let no_edit = client
        .call("edit_page", json!({"id": id, "version": 1}))
        .await;
    assert_eq!(no_edit["isError"], true);
    assert_eq!(no_edit["structuredContent"]["code"], "invalid_arguments");
    for arguments in [
        json!({"version": 1, "markdown": "# Changed"}),
        json!({"id": id, "markdown": "# Changed"}),
        json!({"id": id, "version": 1, "markdown": "# Changed", "access_password": "new"}),
    ] {
        assert_eq!(client.call("edit_page", arguments).await["isError"], true);
    }

    for (version, markdown) in [(1, " \n\n"), (2, "")] {
        let edited = client
            .call(
                "edit_page",
                json!({"id": id, "version": version, "markdown": markdown}),
            )
            .await;
        assert_eq!(edited["structuredContent"]["version"], version + 1);
        let read = client.call("read_page", json!({"id": id})).await;
        assert_eq!(read["structuredContent"]["markdown"], markdown);
    }
}
