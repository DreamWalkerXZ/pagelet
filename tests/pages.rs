use pagelet::{AppState, router};
use serde_json::{Value, json};

#[tokio::test]
async fn publication_is_public_raw_and_persistent() {
    let directory = tempfile::tempdir().unwrap();
    let state = AppState::open(directory.path()).unwrap();
    let token = state.initialize("admin", "a long password").unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{url}/_/api/v1/pages"))
        .bearer_auth(&token)
        .json(&json!({"slug":"hello","markdown":"# Hello 世界"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let page: Value = response.json().await.unwrap();
    assert_eq!(page["version"], 1);
    assert_eq!(page["owner_id"], 1);
    assert_eq!(page["created_at"], page["updated_at"]);
    let response = client
        .get(format!("{url}/_/api/v1/pages/by-slug/hello"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.json::<Value>().await.unwrap(), page);
    let response = client
        .get(format!("{url}/_/raw/hello"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.headers()["content-type"],
        "text/plain; charset=utf-8"
    );
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert_eq!(response.text().await.unwrap(), "# Hello 世界");
    server.abort();
    let state = AppState::open(directory.path()).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "http://{}/_/api/v1/pages/by-slug/hello",
        listener.local_addr().unwrap()
    );
    let server = tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
    assert_eq!(
        client
            .get(url)
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap(),
        page
    );
    server.abort();
}

#[tokio::test]
async fn validation_and_discovery_preserve_owner_boundaries() {
    let directory = tempfile::tempdir().unwrap();
    let state = AppState::open(directory.path()).unwrap();
    let admin = state.initialize("admin", "a long password").unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/_/api/v1", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
    let client = reqwest::Client::new();
    let user: Value = client
        .post(format!("{url}/users"))
        .bearer_auth(&admin)
        .json(&json!({"username":"alice","password":"a long password","role":"user"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let alice = user["token"].as_str().unwrap();
    for method in [reqwest::Method::GET, reqwest::Method::POST] {
        assert_eq!(
            client
                .request(method, format!("{url}/pages"))
                .json(&json!({"markdown":""}))
                .send()
                .await
                .unwrap()
                .status(),
            401
        );
    }
    for slug in [
        "",
        "-a",
        "a-",
        "a--b",
        "Hello",
        "a_b",
        "世界",
        &"a".repeat(65),
    ] {
        assert_eq!(
            client
                .post(format!("{url}/pages"))
                .bearer_auth(alice)
                .json(&json!({"slug":slug,"markdown":""}))
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
    }
    for slug in ["a".to_string(), "a".repeat(64), "a-1".to_string()] {
        assert_eq!(
            client
                .post(format!("{url}/pages"))
                .bearer_auth(alice)
                .json(&json!({"slug":slug,"markdown":""}))
                .send()
                .await
                .unwrap()
                .status(),
            201
        );
    }
    assert_eq!(
        client
            .post(format!("{url}/pages"))
            .bearer_auth(&admin)
            .json(&json!({"slug":"a","markdown":"replace"}))
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    let generated: Value = client
        .post(format!("{url}/pages"))
        .bearer_auth(&admin)
        .json(&json!({"markdown":"admin"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let slug = generated["slug"].as_str().unwrap();
    assert_eq!(slug.len(), 12);
    assert!(
        slug.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    );
    let list: Value = client
        .get(format!("{url}/pages"))
        .bearer_auth(alice)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list["total"], 3);
    assert_eq!(list["per_page"], 20);
    assert_eq!(list["pages"][0]["slug"], "a-1");
    assert!(list["pages"][0].get("markdown").is_none());
    assert_eq!(
        client
            .get(format!("{url}/pages?owner_id=1"))
            .bearer_auth(alice)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let list: Value = client
        .get(format!("{url}/pages?owner_id=2&search=a-&per_page=1"))
        .bearer_auth(&admin)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list["total"], 1);
    assert_eq!(list["pages"][0]["slug"], "a-1");
    for query in [
        "page=0",
        "per_page=0",
        "page=18446744073709551615",
        "owner_id=no",
        "unknown=yes",
    ] {
        assert_eq!(
            client
                .get(format!("{url}/pages?{query}"))
                .bearer_auth(alice)
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
    }
    for (markdown, status) in [
        ("\0".repeat(1024 * 1024), 201),
        ("é".repeat(524288), 201),
        ("é".repeat(524289), 413),
    ] {
        assert_eq!(
            client
                .post(format!("{url}/pages"))
                .bearer_auth(alice)
                .json(&json!({"markdown":markdown}))
                .send()
                .await
                .unwrap()
                .status(),
            status
        );
    }
    assert_eq!(
        client
            .post(format!("{url}/pages"))
            .bearer_auth(alice)
            .header("content-type", "application/json")
            .body(vec![b'{', b'"', 0xff, b'"', b'}'])
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        client
            .post(format!("{url}/pages"))
            .bearer_auth(alice)
            .header("content-type", "application/json")
            .body(" ".repeat(8 * 1024 * 1024 + 1))
            .send()
            .await
            .unwrap()
            .status(),
        413
    );
    assert_eq!(
        client
            .get(format!("{url}/pages/by-slug/missing"))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    server.abort();
}

#[tokio::test]
async fn generated_slug_collisions_retry_without_replacing_content() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = AppState::open(directory.path()).unwrap();
    let token = state.initialize("admin", "a long password").unwrap();
    let slugs = std::sync::Mutex::new(std::collections::VecDeque::from([
        "abcdefghijkl",
        "abcdefghijkl",
        "mnopqrstuvwx",
    ]));
    state.slug_generator =
        std::sync::Arc::new(move || slugs.lock().unwrap().pop_front().unwrap().to_string());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/_/api/v1/pages", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
    let client = reqwest::Client::new();
    for (markdown, slug) in [("first", "abcdefghijkl"), ("second", "mnopqrstuvwx")] {
        let response = client
            .post(&url)
            .bearer_auth(&token)
            .json(&json!({"markdown":markdown}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 201);
        assert_eq!(response.json::<Value>().await.unwrap()["slug"], slug);
    }
    assert_eq!(
        client
            .get(format!("{url}/by-slug/abcdefghijkl"))
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()["markdown"],
        "first"
    );
    server.abort();
}

#[tokio::test]
async fn stable_id_edits_conflicts_and_permanent_deletion() {
    let directory = tempfile::tempdir().unwrap();
    let state = AppState::open(directory.path()).unwrap();
    let token = state.initialize("admin", "a long password").unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/_/api/v1/pages", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
    let client = reqwest::Client::new();
    let original: Value = client
        .post(&url)
        .bearer_auth(&token)
        .json(&json!({"slug":"old","markdown":"first"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let target = format!("{url}/{}", original["id"]);
    let response = client
        .patch(&target)
        .bearer_auth(&token)
        .json(&json!({"version":1,"slug":"new"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let changed: Value = response.json().await.unwrap();
    assert_eq!(changed["id"], original["id"]);
    assert_eq!(changed["markdown"], "first");
    assert_eq!(changed["created_at"], original["created_at"]);
    assert_eq!(changed["version"], 2);
    assert!(changed["updated_at"].as_i64().unwrap() > original["updated_at"].as_i64().unwrap());
    assert_eq!(
        client
            .get(format!("{url}/by-slug/old"))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        client
            .post(&url)
            .bearer_auth(&token)
            .json(&json!({"slug":"old","markdown":"reused"}))
            .send()
            .await
            .unwrap()
            .status(),
        201
    );
    for method in [reqwest::Method::PATCH, reqwest::Method::DELETE] {
        assert_eq!(
            client
                .request(method, &target)
                .bearer_auth(&token)
                .json(&json!({"version":1}))
                .send()
                .await
                .unwrap()
                .status(),
            409
        );
    }
    assert_eq!(
        client
            .delete(&target)
            .bearer_auth(&token)
            .json(&json!({"version":2}))
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        client
            .get(&target)
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        client
            .post(&url)
            .bearer_auth(&token)
            .json(&json!({"slug":"new","markdown":"replacement"}))
            .send()
            .await
            .unwrap()
            .status(),
        201
    );
    assert_eq!(
        client
            .get(&target)
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn page_management_authorization_validation_and_competing_mutations() {
    let directory = tempfile::tempdir().unwrap();
    let state = AppState::open(directory.path()).unwrap();
    let admin = state.initialize("admin", "a long password").unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/_/api/v1", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
    let client = reqwest::Client::new();
    let mut tokens = Vec::new();
    for username in ["alice", "bobby"] {
        let user: Value = client
            .post(format!("{url}/users"))
            .bearer_auth(&admin)
            .json(&json!({"username":username,"password":"a long password","role":"user"}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        tokens.push(user["token"].as_str().unwrap().to_owned());
    }
    let page: Value = client
        .post(format!("{url}/pages"))
        .bearer_auth(&tokens[0])
        .json(&json!({"slug":"mine","markdown":"original"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let target = format!("{url}/pages/{}", page["id"]);
    for method in [
        reqwest::Method::GET,
        reqwest::Method::PATCH,
        reqwest::Method::DELETE,
    ] {
        for (token, expected) in [(None, 401), (Some(&tokens[1]), 403)] {
            let mut request = client
                .request(method.clone(), &target)
                .json(&json!({"version":1,"slug":"stolen"}));
            if method == reqwest::Method::DELETE {
                request = client
                    .request(method.clone(), &target)
                    .json(&json!({"version":1}));
            }
            if let Some(token) = token {
                request = request.bearer_auth(token);
            }
            assert_eq!(request.send().await.unwrap().status(), expected);
        }
    }
    for body in [
        json!({"version":1,"slug":"Bad","markdown":"lost"}),
        json!({"version":0,"markdown":"lost"}),
        json!({"markdown":"lost"}),
        json!({"version":1}),
        json!({"version":1,"owner_id":1}),
    ] {
        assert_eq!(
            client
                .patch(&target)
                .bearer_auth(&tokens[0])
                .json(&body)
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
    }
    client
        .post(format!("{url}/pages"))
        .bearer_auth(&admin)
        .json(&json!({"slug":"taken","markdown":""}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    assert_eq!(
        client
            .patch(&target)
            .bearer_auth(&tokens[0])
            .json(&json!({"version":1,"slug":"taken","markdown":"lost"}))
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    assert_eq!(
        client
            .get(&target)
            .bearer_auth(&admin)
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap(),
        page
    );
    let first = client
        .patch(&target)
        .bearer_auth(&tokens[0])
        .json(&json!({"version":1,"markdown":"first"}))
        .send();
    let second = client
        .patch(&target)
        .bearer_auth(&admin)
        .json(&json!({"version":1,"markdown":"second"}))
        .send();
    let (first, second) = tokio::join!(first, second);
    let mut statuses = [
        first.unwrap().status().as_u16(),
        second.unwrap().status().as_u16(),
    ];
    statuses.sort();
    assert_eq!(statuses, [200, 409]);
    let edit = client
        .patch(&target)
        .bearer_auth(&tokens[0])
        .json(&json!({"version":2,"markdown":"third"}))
        .send();
    let delete = client
        .delete(&target)
        .bearer_auth(&admin)
        .json(&json!({"version":2}))
        .send();
    let (edit, delete) = tokio::join!(edit, delete);
    let (edit, delete) = (
        edit.unwrap().status().as_u16(),
        delete.unwrap().status().as_u16(),
    );
    assert!(
        matches!((edit, delete), (200, 409) | (404, 204)),
        "edit={edit}, delete={delete}"
    );
    let disposable: Value = client
        .post(format!("{url}/pages"))
        .bearer_auth(&admin)
        .json(&json!({"markdown":"delete race"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let disposable_url = format!("{url}/pages/{}", disposable["id"]);
    let first = client
        .delete(&disposable_url)
        .bearer_auth(&admin)
        .json(&json!({"version":1}))
        .send();
    let second = client
        .delete(&disposable_url)
        .bearer_auth(&admin)
        .json(&json!({"version":1}))
        .send();
    let (first, second) = tokio::join!(first, second);
    let mut statuses = [
        first.unwrap().status().as_u16(),
        second.unwrap().status().as_u16(),
    ];
    statuses.sort();
    assert_eq!(statuses, [204, 404]);
    server.abort();
}
