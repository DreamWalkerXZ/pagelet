use pagelet::{AppState, router};
use serde_json::{Value, json};

async fn start(state: AppState) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/_/api/v1", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
    (url, server)
}
async fn user(c: &reqwest::Client, url: &str, admin: &str, name: &str, role: &str) -> Value {
    let r = c
        .post(format!("{url}/users"))
        .bearer_auth(admin)
        .json(&json!({"username":name,"password":"a long password","role":role}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 201);
    r.json().await.unwrap()
}

#[tokio::test]
async fn transfer_preserves_page_revokes_grants_and_allows_account_deletion() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::open(dir.path()).unwrap();
    let admin = state.initialize("admin", "a long password").unwrap();
    let (url, server) = start(state).await;
    let c = reqwest::Client::new();
    let alice = user(&c, &url, &admin, "alice", "user").await;
    let bob = user(&c, &url, &admin, "bob", "user").await;
    let alice_token = alice["token"].as_str().unwrap();
    let page: Value = c
        .post(format!("{url}/pages"))
        .bearer_auth(alice_token)
        .json(&json!({"slug":"secret","markdown":"# Keep me","access_password":"secret"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = page["id"].as_i64().unwrap();
    let alice_id = alice["user"]["id"].as_i64().unwrap();
    let transfer = format!("{url}/pages/{id}/owner");
    let body = json!({"version":1,"owner_id":bob["user"]["id"]});
    assert_eq!(
        c.delete(format!("{url}/users/{alice_id}"))
            .bearer_auth(&admin)
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    for token in [
        None,
        Some(alice_token),
        Some(bob["token"].as_str().unwrap()),
    ] {
        let mut request = c.post(&transfer).json(&body);
        if let Some(t) = token {
            request = request.bearer_auth(t);
        }
        assert_eq!(
            request.send().await.unwrap().status(),
            if token.is_none() { 401 } else { 403 }
        );
    }
    for token in [None, Some(alice_token)] {
        let mut request = c.delete(format!("{url}/users/{alice_id}"));
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        assert_eq!(
            request.send().await.unwrap().status(),
            if token.is_none() { 401 } else { 403 }
        );
    }
    for body in [
        json!({"version":0,"owner_id":1}),
        json!({"version":1,"owner_id":0}),
        json!({"version":1,"owner_id":1,"role":"user"}),
    ] {
        assert_eq!(
            c.post(&transfer)
                .bearer_auth(&admin)
                .json(&body)
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
    }
    assert_eq!(
        c.post(&transfer)
            .bearer_auth(&admin)
            .json(&json!({"version":1,"owner_id":9999}))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    let contract: Value = c
        .get(url.replace("/_/api/v1", "/_/openapi.json"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        contract["paths"]["/_/api/v1/pages/{id}/owner"]["post"]["requestBody"]["content"]["application/json"]
            ["schema"]["$ref"],
        "#/components/schemas/TransferOwnership"
    );
    let public: Value = c
        .post(format!("{url}/pages"))
        .bearer_auth(alice_token)
        .json(&json!({"slug":"public","markdown":"Public content"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        c.post(format!("{url}/pages/{}/owner", public["id"]))
            .bearer_auth(&admin)
            .json(&json!({"version":1,"owner_id":bob["user"]["id"]}))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        c.get(format!("{url}/pages/by-slug/public"))
            .bearer_auth(alice_token)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let login = c
        .post(format!("{url}/login"))
        .header("x-pagelet-csrf", "1")
        .json(&json!({"username":"alice","password":"a long password"}))
        .send()
        .await
        .unwrap();
    assert_eq!(login.status(), 200);
    let cookie = login.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let grant: Value = c
        .post(format!("{url}/pages/by-slug/secret/verify-password"))
        .json(&json!({"password":"secret"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let changed = c
        .post(&transfer)
        .bearer_auth(&admin)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(changed.status(), 200);
    let changed: Value = changed.json().await.unwrap();
    assert_eq!(changed["version"], 2);
    for field in ["id", "slug", "markdown", "created_at"] {
        assert_eq!(changed[field], page[field]);
    }
    assert_eq!(changed["owner_id"], bob["user"]["id"]);
    assert_eq!(
        c.post(&transfer)
            .bearer_auth(&admin)
            .json(&body)
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    assert_eq!(
        c.get(format!("{url}/pages/{id}"))
            .bearer_auth(alice_token)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    for token in [None, Some(alice_token)] {
        let mut r = c
            .get(format!("{url}/pages/by-slug/secret"))
            .header("x-pagelet-grant", grant["grant"].as_str().unwrap());
        if let Some(t) = token {
            r = r.bearer_auth(t);
        }
        assert_eq!(r.send().await.unwrap().status(), 401);
    }
    assert_eq!(
        c.get(format!("{url}/pages/by-slug/secret"))
            .bearer_auth(bob["token"].as_str().unwrap())
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        c.post(format!("{url}/pages/by-slug/secret/verify-password"))
            .json(&json!({"password":"secret"}))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        c.delete(format!("{url}/users/{alice_id}"))
            .bearer_auth(&admin)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        c.get(format!("{url}/me"))
            .bearer_auth(alice_token)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        c.get(format!("{url}/me"))
            .header("cookie", cookie)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        c.delete(format!("{url}/users/1"))
            .bearer_auth(&admin)
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn independent_connections_serialize_deletion_with_ownership_changes() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::open(dir.path()).unwrap();
    let admin = state.initialize("admin", "a long password").unwrap();
    let (url, server) = start(state).await;
    let (other, second_server) = start(AppState::open(dir.path()).unwrap()).await;
    let c = reqwest::Client::new();
    let second = user(&c, &url, &admin, "second", "administrator").await;
    let (a, b) = tokio::join!(
        c.delete(format!("{url}/users/1"))
            .bearer_auth(&admin)
            .send(),
        c.delete(format!("{other}/users/2"))
            .bearer_auth(second["token"].as_str().unwrap())
            .send()
    );
    let statuses = [a.unwrap().status().as_u16(), b.unwrap().status().as_u16()];
    assert!(statuses == [204, 409] || statuses == [409, 204]);
    let surviving = if statuses[0] == 204 {
        second["token"].as_str().unwrap()
    } else {
        &admin
    };
    for i in 0..6 {
        let account = user(&c, &url, surviving, &format!("racer-{i}"), "user").await;
        let id = account["user"]["id"].as_i64().unwrap();
        let token = account["token"].as_str().unwrap();
        let (creation, deletion) = tokio::join!(
            c.post(format!("{url}/pages"))
                .bearer_auth(token)
                .json(&json!({"markdown":"racing creation"}))
                .send(),
            c.delete(format!("{other}/users/{id}"))
                .bearer_auth(surviving)
                .send()
        );
        let statuses = [
            creation.unwrap().status().as_u16(),
            deletion.unwrap().status().as_u16(),
        ];
        assert!(
            statuses == [201, 409] || statuses == [401, 204],
            "{statuses:?}"
        );
        let target = user(&c, &url, surviving, &format!("target-{i}"), "user").await;
        let target_id = target["user"]["id"].as_i64().unwrap();
        let page: Value = c
            .post(format!("{url}/pages"))
            .bearer_auth(surviving)
            .json(&json!({"markdown":"racing transfer"}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let page_id = page["id"].as_i64().unwrap();
        let (transfer, deletion) = tokio::join!(
            c.post(format!("{url}/pages/{page_id}/owner"))
                .bearer_auth(surviving)
                .json(&json!({"owner_id":target_id,"version":1}))
                .send(),
            c.delete(format!("{other}/users/{target_id}"))
                .bearer_auth(surviving)
                .send()
        );
        let statuses = [
            transfer.unwrap().status().as_u16(),
            deletion.unwrap().status().as_u16(),
        ];
        assert!(
            statuses == [200, 409] || statuses == [404, 204],
            "{statuses:?}"
        );
        let persisted: Value = c
            .get(format!("{other}/pages/{page_id}"))
            .bearer_auth(surviving)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(persisted["markdown"], "racing transfer");
        if statuses[0] == 200 {
            assert_eq!(persisted["owner_id"], target_id);
        }
    }
    server.abort();
    second_server.abort();
}
