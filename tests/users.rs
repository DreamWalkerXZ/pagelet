use pagelet::{AppState, router};
use serde_json::{Value, json};

#[tokio::test]
async fn administrators_provision_accounts_with_one_time_tokens() {
    let directory = tempfile::tempdir().unwrap();
    let state = AppState::open(directory.path()).unwrap();
    let token = state.initialize("admin", "a long password").unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/_/api/v1", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
    let client = reqwest::Client::new();
    assert_eq!(
        client
            .get(format!("{url}/users"))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let response = client
        .post(format!("{url}/users"))
        .bearer_auth(&token)
        .json(&json!({"username":"alice","password":" 密码 with spaces ","role":"user"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let created: Value = response.json().await.unwrap();
    assert_eq!(created["user"]["username"], "alice");
    let contract: Value = client
        .get(url.replace("/_/api/v1", "/_/openapi.json"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        contract["paths"]["/_/api/v1/users"]["post"]["responses"]["201"]["content"]["application/json"]
            ["schema"]["$ref"],
        "#/components/schemas/CreatedUser"
    );
    assert_eq!(
        contract["paths"]["/_/api/v1/users"]["get"]["security"],
        json!([{"bearerAuth":[]},{"sessionAuth":[]}])
    );
    let alice = created["token"].as_str().unwrap();
    let me: Value = client
        .get(format!("{url}/me"))
        .bearer_auth(alice)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(me, created["user"]);
    assert!(me.get("token").is_none());
    for method in [reqwest::Method::GET, reqwest::Method::POST] {
        assert_eq!(
            client
                .request(method, format!("{url}/users"))
                .bearer_auth(alice)
                .json(
                    &json!({"username":"other","password":"a long password","role":"administrator"})
                )
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
    }
    let list: Value = client
        .get(format!("{url}/users"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list["total"], 2);
    assert_eq!(list["users"][1], created["user"]);
    let extra = client
        .post(format!("{url}/users"))
        .bearer_auth(&token)
        .json(
            &json!({"username":"second-admin","password":"a long password","role":"administrator"}),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(extra.status(), 201);
    let extra: Value = extra.json().await.unwrap();
    assert_eq!(
        client
            .get(format!("{url}/users"))
            .bearer_auth(extra["token"].as_str().unwrap())
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let old_id = extra["user"]["id"].as_i64().unwrap();
    assert_eq!(
        client
            .delete(format!("{url}/users/{old_id}"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    let replacement: Value = client
        .post(format!("{url}/users"))
        .bearer_auth(&token)
        .json(&json!({"username":"second-admin","password":"another password","role":"administrator"}))
        .send().await.unwrap().json().await.unwrap();
    assert!(replacement["user"]["id"].as_i64().unwrap() > old_id);
    assert_eq!(
        client
            .post(format!("{url}/users/{old_id}/token"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    server.abort();
}

#[tokio::test]
async fn user_validation_search_and_pagination_match_the_contract() {
    let directory = tempfile::tempdir().unwrap();
    let state = AppState::open(directory.path()).unwrap();
    let token = state.initialize("admin", "a long password").unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/_/api/v1/users", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
    let client = reqwest::Client::new();
    for username in [
        "ab",
        "Admin",
        "-alice",
        "alice-",
        "ali--ce",
        "éclair",
        &"a".repeat(33),
    ] {
        assert_eq!(
            client
                .post(&url)
                .bearer_auth(&token)
                .json(&json!({"username":username,"password":"a long password","role":"user"}))
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
    }
    for password in ["a".repeat(11), "密".repeat(129)] {
        assert_eq!(
            client
                .post(&url)
                .bearer_auth(&token)
                .json(&json!({"username":"alice","password":password,"role":"user"}))
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
    }
    for role in ["admin", "ordinary-user", "USER"] {
        assert_eq!(
            client
                .post(&url)
                .bearer_auth(&token)
                .json(&json!({"username":"alice","password":"a long password","role":role}))
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
    }
    for (username, password) in [
        ("alice".to_string(), "密".repeat(12)),
        ("alice-2".to_string(), " ".repeat(128)),
        ("a".repeat(32), "x".repeat(12)),
    ] {
        assert_eq!(
            client
                .post(&url)
                .bearer_auth(&token)
                .json(&json!({"username":username,"password":password,"role":"user"}))
                .send()
                .await
                .unwrap()
                .status(),
            201
        );
    }
    assert_eq!(
        client
            .post(&url)
            .bearer_auth(&token)
            .json(&json!({"username":"alice","password":"a long password","role":"administrator"}))
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    let page: Value = client
        .get(&url)
        .bearer_auth(&token)
        .query(&[("search", "alice"), ("page", "2"), ("per_page", "1")])
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        page,
        json!({"users":[{"id":3,"username":"alice-2","role":"user"}],"total":2,"page":2,"per_page":1})
    );
    let defaults: Value = client
        .get(&url)
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(defaults["per_page"], 20);
    let capped: Value = client
        .get(&url)
        .bearer_auth(&token)
        .query(&[("per_page", "101")])
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(capped["per_page"], 100);
    for query in [
        "page=0",
        "per_page=0",
        "page=-1",
        "page=18446744073709551615",
        "unknown=1",
    ] {
        assert_eq!(
            client
                .get(format!("{url}?{query}"))
                .bearer_auth(&token)
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
    }
    let literal: Value = client
        .get(&url)
        .bearer_auth(&token)
        .query(&[("search", "%")])
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(literal["total"], 0);
    for method in [reqwest::Method::PATCH, reqwest::Method::PUT] {
        assert_eq!(
            client
                .request(method, format!("{url}/2"))
                .bearer_auth(&token)
                .json(&json!({"username":"renamed","role":"administrator"}))
                .send()
                .await
                .unwrap()
                .status(),
            405
        );
    }
    server.abort();
}
