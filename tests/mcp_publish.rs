#[allow(dead_code)]
#[path = "mcp/support.rs"]
mod support;

use serde_json::{Value, json};
use support::TestServer;

#[tokio::test]
async fn publishes_a_named_page_as_an_ordinary_user() {
    let server = TestServer::start(true, None).await;
    let alice = server.user("alice").await;
    let client = server.connect(&alice).await;
    let tools = client.tools().await;
    let publish = tools
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "publish_page")
        .unwrap();
    assert_eq!(publish["annotations"]["readOnlyHint"], false);
    assert_eq!(publish["inputSchema"]["required"], json!(["markdown"]));
    assert_eq!(publish["inputSchema"]["additionalProperties"], false);

    let result = client
        .call(
            "publish_page",
            json!({"markdown": "# Published 世界", "slug": "hello"}),
        )
        .await;
    let content = &result["structuredContent"];
    assert!(content["id"].as_i64().is_some());
    assert_eq!(
        content,
        &json!({"id": content["id"], "slug": "hello", "version": 1, "url": "/hello"})
    );
    let page: Value = server
        .client
        .get(format!("{}/_/api/v1/pages/by-slug/hello", server.base_url))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(page["id"], content["id"]);
    assert_eq!(page["markdown"], "# Published 世界");
}

#[tokio::test]
async fn publishes_with_a_generated_slug_and_an_external_reader_link() {
    let server = TestServer::start(true, Some("https://pages.example:8443/configured/path")).await;
    let alice = server.user("alice").await;
    let client = server.connect(&alice).await;

    let result = client
        .call("publish_page", json!({"markdown": "# Generated"}))
        .await;
    let content = &result["structuredContent"];
    let slug = content["slug"].as_str().unwrap();
    assert!(!slug.is_empty());
    assert_eq!(content["version"], 1);
    assert_eq!(content["url"], format!("https://pages.example:8443/{slug}"));

    let reader_url = reqwest::Url::parse(content["url"].as_str().unwrap()).unwrap();
    assert_eq!(
        server
            .client
            .get(format!("{}{}", server.base_url, reader_url.path()))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let raw = server
        .client
        .get(format!("{}/_/raw/{slug}", server.base_url))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .text()
        .await
        .unwrap();
    assert_eq!(raw, "# Generated");
}

#[tokio::test]
async fn publishing_an_occupied_slug_preserves_the_existing_page() {
    let server = TestServer::start(true, None).await;
    let alice = server.user("alice").await;
    let existing = server
        .publish(&server.admin, "occupied", "# Existing")
        .await;
    let client = server.connect(&alice).await;

    let result = client
        .call(
            "publish_page",
            json!({"markdown": "# Replacement", "slug": "occupied"}),
        )
        .await;
    assert_eq!(result["isError"], true);
    assert_eq!(result["structuredContent"], json!({"code": "slug_taken"}));

    let page: Value = server
        .client
        .get(format!(
            "{}/_/api/v1/pages/by-slug/occupied",
            server.base_url
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(page, existing);
}

#[tokio::test]
async fn applies_publication_validation_and_rejects_an_access_password() {
    let server = TestServer::start(true, None).await;
    let alice = server.user("alice").await;
    let client = server.connect(&alice).await;

    let invalid_slug = client
        .call("publish_page", json!({"markdown": "", "slug": "Bad Slug"}))
        .await;
    assert_eq!(invalid_slug["isError"], true);
    assert_eq!(
        invalid_slug["structuredContent"],
        json!({"code": "invalid_arguments"})
    );
    let long_markdown = client
        .call(
            "publish_page",
            json!({"markdown": "a".repeat(1024 * 1024 + 1)}),
        )
        .await;
    assert_eq!(long_markdown["isError"], true);
    assert_eq!(
        long_markdown["structuredContent"],
        json!({"code": "payload_too_large"})
    );
    let password = client
        .call(
            "publish_page",
            json!({"markdown": "", "access_password": "unaccepted"}),
        )
        .await;
    assert_eq!(password["isError"], true);
}
