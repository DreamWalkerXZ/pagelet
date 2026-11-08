use pagelet::{AppState, router};
use serde_json::{Value, json};

#[tokio::test]
async fn sessions_are_separate_credentials_and_logout_revokes_only_current_session() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::open(dir.path()).unwrap();
    let token = state.initialize("admin", "a long password").unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/_/api/v1", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
    let c = reqwest::Client::new();
    assert_eq!(
        c.post(format!("{url}/login"))
            .json(&json!({"username":"admin","password":"a long password"}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let r = c
        .post(format!("{url}/login"))
        .header("x-pagelet-csrf", "1")
        .json(&json!({"username":"admin","password":"a long password"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let cookie = r.headers()["set-cookie"].to_str().unwrap().to_owned();
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("Max-Age=604800"));
    let body: Value = r.json().await.unwrap();
    assert!(body.get("token").is_none());
    let cookie = cookie.split(';').next().unwrap();
    assert_eq!(
        c.get(format!("{url}/me"))
            .header("cookie", cookie)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        c.post(format!("{url}/logout"))
            .header("cookie", cookie)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        c.post(format!("{url}/logout"))
            .header("cookie", cookie)
            .header("x-pagelet-csrf", "1")
            .send()
            .await
            .unwrap()
            .status(),
        204
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
        c.get(format!("{url}/me"))
            .bearer_auth(token)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    server.abort();
}

#[test]
fn local_recovery_rotates_administrator_credentials() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::open(dir.path()).unwrap();
    state.initialize("admin", "a long password").unwrap();
    drop(state);
    let mut child = Command::new(env!("CARGO_BIN_EXE_pagelet"))
        .args(["--data-dir", dir.path().to_str().unwrap(), "recover-admin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"admin\na new password\na new password\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("API token:"));
}

#[tokio::test]
async fn password_changes_revoke_sessions_and_tokens_but_token_reset_keeps_sessions() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = AppState::open(dir.path()).unwrap();
    let token = state.initialize("admin", "a long password").unwrap();
    let time = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(1000));
    let clock = time.clone();
    state.clock = std::sync::Arc::new(move || clock.load(std::sync::atomic::Ordering::SeqCst));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/_/api/v1", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            router(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap()
    });
    let c = reqwest::Client::new();
    let mut cookies = Vec::new();
    for _ in 0..2 {
        let r = c
            .post(format!("{url}/login"))
            .header("x-pagelet-csrf", "1")
            .json(&json!({"username":"admin","password":"a long password"}))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        cookies.push(
            r.headers()["set-cookie"]
                .to_str()
                .unwrap()
                .split(';')
                .next()
                .unwrap()
                .to_string(),
        );
    }
    let reset: Value = c
        .post(format!("{url}/me/token"))
        .header("cookie", &cookies[0])
        .header("x-pagelet-csrf", "1")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        c.get(format!("{url}/me"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        c.get(format!("{url}/me"))
            .header("cookie", &cookies[1])
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        c.get(format!("{url}/me"))
            .header("cookie", &cookies[1])
            .bearer_auth("invalid")
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    for password in ["short".to_string(), "密".repeat(129)] {
        assert_eq!(
            c.post(format!("{url}/me/password"))
                .bearer_auth(reset["token"].as_str().unwrap())
                .json(&json!({"password":password,"current_password":"a long password"}))
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
    }
    assert_eq!(
        c.post(format!("{url}/me/password"))
            .header("cookie", &cookies[0])
            .header("x-pagelet-csrf", "1")
            .json(&json!({"password":"a new password","current_password":"incorrect"}))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let change = c
        .post(format!("{url}/me/password"))
        .header("cookie", &cookies[0])
        .header("x-pagelet-csrf", "1")
        .json(&json!({"password":"a new password","current_password":"a long password"}))
        .send()
        .await
        .unwrap();
    assert_eq!(change.status(), 200);
    let change: Value = change.json().await.unwrap();
    for cookie in &cookies {
        assert_eq!(
            c.get(format!("{url}/me"))
                .header("cookie", cookie)
                .send()
                .await
                .unwrap()
                .status(),
            401
        );
    }
    assert_eq!(
        c.get(format!("{url}/me"))
            .bearer_auth(reset["token"].as_str().unwrap())
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        c.get(format!("{url}/me"))
            .bearer_auth(change["token"].as_str().unwrap())
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let r = c
        .post(format!("{url}/login"))
        .header("x-pagelet-csrf", "1")
        .json(&json!({"username":"admin","password":"a new password"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let cookie = r.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    time.store(605799, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        c.get(format!("{url}/me"))
            .header("cookie", &cookie)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    time.store(605800, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        c.get(format!("{url}/me"))
            .header("cookie", cookie)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    server.abort();
}

#[tokio::test]
async fn login_rate_limit_expires_without_permanent_account_lockout() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = AppState::open(dir.path()).unwrap();
    state.initialize("admin", "a long password").unwrap();
    let time = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(1000));
    let clock = time.clone();
    state.clock = std::sync::Arc::new(move || clock.load(std::sync::atomic::Ordering::SeqCst));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/_/api/v1", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            router(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap()
    });
    let c = reqwest::Client::new();
    for n in 0..11 {
        assert_eq!(
            c.post(format!("{url}/login"))
                .header("x-pagelet-csrf", "1")
                .header("x-forwarded-for", format!("192.0.2.{n}"))
                .json(&json!({"username":"admin","password":"wrong"}))
                .send()
                .await
                .unwrap()
                .status(),
            if n == 10 { 429 } else { 401 }
        );
    }
    time.store(1060, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        c.post(format!("{url}/login"))
            .header("x-pagelet-csrf", "1")
            .json(&json!({"username":"admin","password":"a long password"}))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    server.abort();
}

#[tokio::test]
async fn administrators_can_reset_credentials_and_cookie_mutations_require_csrf() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::open(dir.path()).unwrap();
    let admin = state.initialize("admin", "a long password").unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/_/api/v1", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
    let c = reqwest::Client::new();
    let alice: Value = c
        .post(format!("{url}/users"))
        .bearer_auth(&admin)
        .json(&json!({"username":"alice","password":"alice password","role":"user"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let alice_token = alice["token"].as_str().unwrap();
    let id = alice["user"]["id"].as_i64().unwrap();
    assert_eq!(
        c.post(format!("{url}/users/1/password"))
            .bearer_auth(alice_token)
            .json(&json!({"password":"changed password"}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        c.post(format!("{url}/users/1/token"))
            .bearer_auth(alice_token)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let r = c
        .post(format!("{url}/login"))
        .header("x-pagelet-csrf", "1")
        .json(&json!({"username":"admin","password":"a long password"}))
        .send()
        .await
        .unwrap();
    let cookie = r.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    assert_eq!(
        c.post(format!("{url}/users"))
            .header("cookie", cookie)
            .json(&json!({"username":"other","password":"a long password","role":"user"}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let reset: Value = c
        .post(format!("{url}/users/{id}/password"))
        .bearer_auth(&admin)
        .json(&json!({"password":"changed password"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(reset["token"].is_string());
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
            .bearer_auth(reset["token"].as_str().unwrap())
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let own: Value = c
        .post(format!("{url}/users/{id}/token"))
        .bearer_auth(reset["token"].as_str().unwrap())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let rotated: Value = c
        .post(format!("{url}/users/{id}/token"))
        .bearer_auth(&admin)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_ne!(own["token"], rotated["token"]);
    assert_eq!(
        c.get(format!("{url}/me"))
            .bearer_auth(own["token"].as_str().unwrap())
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        c.post(format!("{url}/users/999/password"))
            .bearer_auth(admin)
            .json(&json!({"password":"changed password"}))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    server.abort();
}

#[tokio::test]
async fn older_instances_upgrade_pages_and_sessions_without_losing_credentials() {
    for version in [1, 2, 3, 4] {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::open(dir.path()).unwrap();
        let token = state.initialize("admin", "a long password").unwrap();
        drop(state);
        // Recreate the previous release's schema as an upgrade fixture.
        let db = rusqlite::Connection::open(dir.path().join("pagelet.sqlite3")).unwrap();
        db.execute_batch("DROP TABLE user_id_sequence;").unwrap();
        if version < 4 {
            db.execute_batch("DROP TABLE page_grants;").unwrap();
        }
        if version < 3 {
            db.execute_batch("DROP TABLE sessions;").unwrap();
        }
        if version == 1 {
            db.execute_batch("DROP TABLE pages;").unwrap();
        }
        db.pragma_update(None, "user_version", version).unwrap();
        drop(db);
        let state = AppState::open(dir.path()).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/_/api/v1", listener.local_addr().unwrap());
        let server =
            tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
        let c = reqwest::Client::new();
        assert_eq!(
            c.post(format!("{url}/pages"))
                .bearer_auth(token)
                .json(&json!({"slug":"upgraded","markdown":"kept"}))
                .send()
                .await
                .unwrap()
                .status(),
            201
        );
        let r = c
            .post(format!("{url}/login"))
            .header("x-pagelet-csrf", "1")
            .json(&json!({"username":"admin","password":"a long password"}))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        let cookie = r.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();
        let reopened = AppState::open(dir.path()).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let after = format!("http://{}/_/api/v1", listener.local_addr().unwrap());
        let restarted =
            tokio::spawn(async move { axum::serve(listener, router(reopened)).await.unwrap() });
        assert_eq!(
            c.get(format!("{after}/me"))
                .header("cookie", cookie)
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
        assert_eq!(
            c.get(format!("{after}/pages/by-slug/upgraded"))
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
        server.abort();
        restarted.abort();
    }
}
