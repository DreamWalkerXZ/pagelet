use pagelet::{AppState, router};
use serde_json::{Value, json};

#[tokio::test]
async fn protected_reads_share_page_scoped_expiring_grants() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = AppState::open(dir.path()).unwrap();
    let token = state.initialize("admin", "a long password").unwrap();
    let time = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(1000));
    let clock = time.clone();
    state.clock = std::sync::Arc::new(move || clock.load(std::sync::atomic::Ordering::SeqCst));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            router(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap()
    });
    let c = reqwest::Client::new();
    let mut accounts = Vec::new();
    for username in ["owner", "other"] {
        let user: Value = c
            .post(format!("{url}/_/api/v1/users"))
            .bearer_auth(&token)
            .json(&json!({"username":username,"password":"a long password","role":"user"}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        accounts.push(user["token"].as_str().unwrap().to_owned());
    }
    let r = c
        .post(format!("{url}/_/api/v1/pages"))
        .bearer_auth(&accounts[0])
        .json(&json!({"slug":"private","markdown":"secret","access_password":" 密 "}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 201);
    let page: Value = r.json().await.unwrap();
    for path in ["/_/api/v1/pages/by-slug/private", "/_/raw/private"] {
        let r = c.get(format!("{url}{path}")).send().await.unwrap();
        assert_eq!(r.status(), 401);
        assert_eq!(r.headers()["cache-control"], "no-store");
        assert_eq!(
            r.json::<Value>().await.unwrap(),
            json!({"error":"password_required"})
        );
        assert_eq!(
            c.get(format!("{url}{path}"))
                .bearer_auth(&token)
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
    }
    for path in ["/_/api/v1/pages/by-slug/private", "/_/raw/private"] {
        assert_eq!(
            c.get(format!("{url}{path}"))
                .bearer_auth(&accounts[0])
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
        let r = c
            .get(format!("{url}{path}"))
            .bearer_auth(&accounts[1])
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 401);
        assert_eq!(
            r.json::<Value>().await.unwrap(),
            json!({"error":"password_required"})
        );
    }
    let verify = format!("{url}/_/api/v1/pages/by-slug/private/verify-password");
    assert_eq!(
        c.post(&verify)
            .json(&json!({"password":"密"}))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let r = c
        .post(&verify)
        .json(&json!({"password":" 密 "}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let cookie = r.headers()["set-cookie"].to_str().unwrap().to_owned();
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("Max-Age=86400"));
    let grant: Value = r.json().await.unwrap();
    assert_eq!(grant["expires_at"], 87400);
    let raw = format!("{url}/_/raw/private");
    assert_eq!(
        c.get(&raw)
            .header("x-pagelet-grant", grant["grant"].as_str().unwrap())
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
        "secret"
    );
    assert_eq!(
        c.get(&raw)
            .header("cookie", cookie.split(';').next().unwrap())
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        c.patch(format!("{url}/_/api/v1/pages/{}", page["id"]))
            .header("x-pagelet-grant", grant["grant"].as_str().unwrap())
            .json(&json!({"version":1,"markdown":"bad"}))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let contract: Value = c
        .get(format!("{url}/_/openapi.json"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        contract["components"]["securitySchemes"]["pageGrant"]["name"],
        "X-Pagelet-Grant"
    );
    assert!(
        contract["paths"]["/_/api/v1/pages/by-slug/{slug}/verify-password"]["post"]["responses"]
            .get("429")
            .is_some()
    );
    let mut reopened = AppState::open(dir.path()).unwrap();
    let clock = time.clone();
    reopened.clock = std::sync::Arc::new(move || clock.load(std::sync::atomic::Ordering::SeqCst));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let restarted_raw = format!("http://{}/_/raw/private", listener.local_addr().unwrap());
    let restarted =
        tokio::spawn(async move { axum::serve(listener, router(reopened)).await.unwrap() });
    assert_eq!(
        c.get(&restarted_raw)
            .header("x-pagelet-grant", grant["grant"].as_str().unwrap())
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    time.store(87400, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        c.get(&raw)
            .header("x-pagelet-grant", grant["grant"].as_str().unwrap())
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    restarted.abort();
    server.abort();
}

#[tokio::test]
async fn policy_changes_revoke_grants_and_slug_reuse_does_not_restore_them() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::open(dir.path()).unwrap();
    let token = state.initialize("admin", "a long password").unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
    let c = reqwest::Client::new();
    let api = format!("{url}/_/api/v1/pages");
    let mut pages = Vec::new();
    let mut cookies = Vec::new();
    let mut grants = Vec::new();
    for slug in ["first", "second"] {
        let r = c
            .post(&api)
            .bearer_auth(&token)
            .json(&json!({"slug":slug,"markdown":"secret","access_password":"p"}))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 201);
        pages.push(r.json::<Value>().await.unwrap());
        let r = c
            .post(format!("{api}/by-slug/{slug}/verify-password"))
            .json(&json!({"password":"p"}))
            .send()
            .await
            .unwrap();
        cookies.push(
            r.headers()["set-cookie"]
                .to_str()
                .unwrap()
                .split(';')
                .next()
                .unwrap()
                .to_owned(),
        );
        grants.push(
            r.json::<Value>().await.unwrap()["grant"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }
    let cookies = cookies.join("; ");
    for slug in ["first", "second"] {
        assert_eq!(
            c.get(format!("{url}/_/raw/{slug}"))
                .header("cookie", &cookies)
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
    }
    assert_eq!(
        c.get(format!("{url}/_/raw/second"))
            .header("x-pagelet-grant", &grants[0])
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let id = pages[0]["id"].as_i64().unwrap();
    assert_eq!(
        c.patch(format!("{api}/{id}"))
            .bearer_auth(&token)
            .json(&json!({"version":1,"access_password":"new"}))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        c.get(format!("{url}/_/raw/first"))
            .header("cookie", &cookies)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        c.patch(format!("{api}/{id}"))
            .bearer_auth(&token)
            .json(&json!({"version":1,"access_password":""}))
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    let r = c
        .post(format!("{api}/by-slug/first/verify-password"))
        .json(&json!({"password":"new"}))
        .send()
        .await
        .unwrap();
    let second_grant = r.json::<Value>().await.unwrap()["grant"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        c.patch(format!("{api}/{id}"))
            .bearer_auth(&token)
            .json(&json!({"version":2,"access_password":""}))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        c.get(format!("{url}/_/raw/first"))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        c.patch(format!("{api}/{id}"))
            .bearer_auth(&token)
            .json(&json!({"version":3,"access_password":"new"}))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        c.get(format!("{url}/_/raw/first"))
            .header("x-pagelet-grant", second_grant)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        c.delete(format!("{api}/{id}"))
            .bearer_auth(&token)
            .json(&json!({"version":4}))
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    let replacement: Value = c
        .post(&api)
        .bearer_auth(&token)
        .json(&json!({"slug":"first","markdown":"replacement","access_password":"p"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_ne!(replacement["id"], pages[0]["id"]);
    assert_eq!(
        c.get(format!("{url}/_/raw/first"))
            .header("x-pagelet-grant", &grants[0])
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        c.get(format!("{url}/_/raw/second"))
            .header("cookie", cookies)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    server.abort();
}

#[tokio::test]
async fn password_validation_and_rate_limits_preserve_exact_unicode_passwords() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = AppState::open(dir.path()).unwrap();
    let token = state.initialize("admin", "a long password").unwrap();
    let time = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(1000));
    let clock = time.clone();
    state.clock = std::sync::Arc::new(move || clock.load(std::sync::atomic::Ordering::SeqCst));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            router(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap()
    });
    let c = reqwest::Client::new();
    let api = format!("{url}/_/api/v1/pages");
    assert_eq!(
        c.post(&api)
            .bearer_auth(&token)
            .json(&json!({"slug":"invalid","markdown":"","access_password":"密".repeat(129)}))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    let password = "密".repeat(128);
    let r = c
        .post(&api)
        .bearer_auth(&token)
        .json(&json!({"slug":"unicode","markdown":"","access_password":password}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 201);
    let page: Value = r.json().await.unwrap();
    assert!(page.get("access_password_hash").is_none());
    assert_eq!(
        c.patch(format!("{api}/{}", page["id"]))
            .bearer_auth(&token)
            .json(&json!({"version":1,"access_password":"密".repeat(129)}))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    for n in 0..11 {
        assert_eq!(
            c.post(format!("{api}/by-slug/unicode/verify-password"))
                .header("x-forwarded-for", format!("192.0.2.{n}"))
                .json(&json!({"password":"wrong"}))
                .send()
                .await
                .unwrap()
                .status(),
            if n == 10 { 429 } else { 401 }
        );
    }
    time.store(1060, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        c.post(format!("{api}/by-slug/unicode/verify-password"))
            .json(&json!({"password":password}))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    // Credentials in URLs never authorize reads.
    assert_eq!(
        c.get(format!("{url}/_/raw/unicode?password={password}"))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    server.abort();
}
