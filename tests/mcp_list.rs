#[path = "mcp/support.rs"]
mod support;

use serde_json::json;
use support::TestServer;

#[tokio::test]
async fn lists_searchable_manageable_metadata_with_rest_pagination() {
    let server = TestServer::start(true, None).await;
    let alice = server.user("alice").await;
    let bob = server.user("bob").await;
    let first = server.publish(&alice, "guide-first", "# First").await;
    let second = server.publish(&alice, "guide-second", "# Second").await;
    server.publish(&alice, "unrelated", "# Other").await;
    let other = server.publish(&bob, "guide-other", "# Bob").await;

    let owner = server.connect(&alice).await;
    let tools = owner.tools().await;
    let list = tools
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "list_pages")
        .unwrap();
    assert_eq!(list["annotations"]["readOnlyHint"], true);
    assert_eq!(
        list["inputSchema"]["properties"]["search"]["type"]
            .as_array()
            .unwrap()[0],
        "string"
    );
    let result = owner
        .call(
            "list_pages",
            json!({"search":"guide", "page":2, "per_page":1}),
        )
        .await;
    assert_eq!(
        result["structuredContent"],
        json!({
            "pages": [{
                "id": first["id"], "slug": "guide-first", "owner_id": first["owner_id"],
                "created_at": first["created_at"], "updated_at": first["updated_at"], "version": 1
            }],
            "total": 2, "page": 2, "per_page": 1
        })
    );
    assert_eq!(
        owner
            .call("list_pages", json!({"owner_id": other["owner_id"]}))
            .await["isError"],
        true
    );
    let admin = server.connect(&server.admin).await;
    let all = admin.call("list_pages", json!({"search":"guide"})).await;
    assert_eq!(all["structuredContent"]["total"], 3);
    let filtered = admin
        .call(
            "list_pages",
            json!({"search":"guide", "owner_id": second["owner_id"]}),
        )
        .await;
    assert_eq!(filtered["structuredContent"]["total"], 2);
    assert_eq!(
        filtered["structuredContent"]["pages"][0]["id"],
        second["id"]
    );
}

#[tokio::test]
async fn endpoint_requires_enablement_allowed_host_and_origin_and_current_bearer_token() {
    let ping = json!({"jsonrpc": "2.0", "id": 1, "method": "ping"});
    let disabled = TestServer::start(false, None).await;
    assert_eq!(
        disabled
            .mcp_request(ping.clone())
            .bearer_auth(&disabled.admin)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    let local = TestServer::start(true, None).await;
    for (host, status) in [
        ("localhost", 200),
        ("[::1]", 200),
        ("pagelet.internal", 403),
    ] {
        assert_eq!(
            local
                .mcp_request(ping.clone())
                .bearer_auth(&local.admin)
                .header("Host", host)
                .send()
                .await
                .unwrap()
                .status(),
            status,
            "{host}"
        );
    }
    assert_eq!(
        local
            .mcp_request(ping.clone())
            .bearer_auth(&local.admin)
            .header("Origin", &local.base_url)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let server = TestServer::start(true, Some("https://pages.example")).await;
    let alice = server.user("alice").await;
    let client = server.connect(&alice).await;
    assert!(client.tools().await.is_array());
    assert_eq!(
        server
            .mcp_request(ping.clone())
            .bearer_auth(&alice)
            .header("Host", "pages.example")
            .header("Origin", "https://pages.example")
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        server
            .mcp_request(ping.clone())
            .bearer_auth(&alice)
            .header("Host", "elsewhere.example")
            .header("X-Forwarded-Host", "pages.example")
            .header("Origin", "https://pages.example")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    for origin in [
        "https://elsewhere.example",
        "https://pages.example:8443",
        "https://pages.example/path",
        "null",
    ] {
        assert_eq!(
            server
                .mcp_request(ping.clone())
                .bearer_auth(&alice)
                .header("Origin", origin)
                .send()
                .await
                .unwrap()
                .status(),
            403,
            "{origin}"
        );
    }
    assert_eq!(
        server
            .mcp_request(ping.clone())
            .bearer_auth(&alice)
            .header("Origin", "https://pages.example")
            .header("Origin", "https://elsewhere.example")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );

    let cookie = server
        .client
        .post(format!("{}/_/api/v1/login", server.base_url))
        .json(&json!({"username":"alice", "password":"a long password"}))
        .header("x-pagelet-csrf", "1")
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let list = json!({"jsonrpc":"2.0", "id":1, "method":"tools/list"});
    let call =
        json!({"jsonrpc":"2.0", "id":1, "method":"tools/call", "params":{"name":"list_pages"}});
    for payload in [&list, &call] {
        assert_eq!(
            server
                .mcp_request(payload.clone())
                .send()
                .await
                .unwrap()
                .status(),
            401
        );
        assert_eq!(
            server
                .mcp_request(payload.clone())
                .header("Cookie", &cookie)
                .send()
                .await
                .unwrap()
                .status(),
            401
        );
    }
    let reset: serde_json::Value = server
        .client
        .post(format!("{}/_/api/v1/me/token", server.base_url))
        .bearer_auth(&alice)
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
            .mcp_request(call)
            .bearer_auth(&alice)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert!(
        server
            .connect(reset["token"].as_str().unwrap())
            .await
            .tools()
            .await
            .is_array()
    );
}

#[tokio::test]
async fn lists_pages_with_stateless_protocol_metadata() {
    let server = TestServer::start(true, None).await;
    let alice = server.user("alice").await;
    let page = server.publish(&alice, "guide", "# Guide").await;
    let metadata = json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientInfo": {"name":"pagelet-test", "version":"1"},
        "io.modelcontextprotocol/clientCapabilities": {}
    });
    for (method, params) in [
        ("server/discover", json!({"_meta": metadata})),
        ("tools/list", json!({"_meta": metadata})),
        (
            "tools/call",
            json!({"_meta": metadata, "name":"list_pages", "arguments":{}}),
        ),
    ] {
        let mut request = server
            .client
            .post(format!("{}/_/mcp", server.base_url))
            .bearer_auth(&alice)
            .header("Accept", "application/json, text/event-stream")
            .header("MCP-Protocol-Version", "2026-07-28")
            .header("Mcp-Method", method);
        if method == "tools/call" {
            request = request.header("Mcp-Name", "list_pages");
        }
        let response: serde_json::Value = request
            .json(&json!({"jsonrpc":"2.0", "id":1, "method":method, "params":params}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        let result = &response["result"];
        match method {
            "server/discover" => assert!(result["capabilities"]["tools"].is_object(), "{response}"),
            "tools/list" => assert!(
                result["tools"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|tool| tool["name"] == "list_pages")
            ),
            _ => assert_eq!(
                result["structuredContent"]["pages"][0]["id"], page["id"],
                "{response}"
            ),
        }
    }
}
