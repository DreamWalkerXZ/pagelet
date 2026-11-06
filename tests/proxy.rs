use pagelet::{AppState, Deployment, router};
use serde_json::json;

#[tokio::test]
async fn trusted_chains_preserve_source_and_target_limits() {
    for (trust, chain, separate) in [
        ("", "192.0.2.1", false),
        ("127.0.0.1", "192.0.2.1", true),
        ("127.0.0.0/8,10.0.0.0/8", "192.0.2.1, 10.1.2.3", true),
        ("127.0.0.1", "192.0.2.1, 198.51.100.1", false),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut state = AppState::open(dir.path()).unwrap();
        state.deployment =
            Deployment::new(None, trust.split(',').filter(|s| !s.is_empty())).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/_/api/v1/login", listener.local_addr().unwrap());
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
            let r = c
                .post(&url)
                .header("x-pagelet-csrf", "1")
                .header("x-forwarded-for", chain)
                .json(&json!({"username":format!("missing{n}"),"password":"wrong"}))
                .send()
                .await
                .unwrap();
            assert_eq!(r.status(), if n == 10 { 429 } else { 401 });
        }
        let other = chain.replace("192.0.2.1", "192.0.2.2");
        let r = c
            .post(&url)
            .header("x-pagelet-csrf", "1")
            .header("x-forwarded-for", other)
            .json(&json!({"username":"different","password":"wrong"}))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), if separate { 401 } else { 429 });
        server.abort();
    }
}

#[tokio::test]
async fn proxy_target_limits_and_secure_cookies_apply_to_both_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = AppState::open(dir.path()).unwrap();
    state.deployment = Deployment::new(
        Some("https://pages.example"),
        ["127.0.0.0/8", "2001:db8::/32"],
    )
    .unwrap();
    let token = state.initialize("admin", "a long password").unwrap();
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
    let login = format!("{url}/_/api/v1/login");
    let r = c
        .post(&login)
        .header("x-pagelet-csrf", "1")
        .json(&json!({"username":"admin","password":"a long password"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let cookie = r.headers()["set-cookie"].to_str().unwrap().to_owned();
    assert!(cookie.contains("; Secure"));
    assert_eq!(
        c.post(format!("{url}/_/api/v1/logout"))
            .header("cookie", cookie.split(';').next().unwrap())
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let r = c
        .post(format!("{url}/_/api/v1/logout"))
        .header("x-pagelet-csrf", "1")
        .header("cookie", cookie.split(';').next().unwrap())
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 204);
    assert!(
        r.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .contains("Max-Age=0; Secure")
    );
    c.post(format!("{url}/_/api/v1/pages"))
        .bearer_auth(&token)
        .json(&json!({"slug":"private","markdown":"secret","access_password":"p"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let verify = format!("{url}/_/api/v1/pages/by-slug/private/verify-password");
    let r = c
        .post(&verify)
        .header("x-forwarded-for", "192.0.2.1, 2001:db8::1")
        .json(&json!({"password":"p"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    assert!(
        r.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .contains("; Secure")
    );
    for n in 0..11 {
        let slug = format!("public-{n}");
        c.post(format!("{url}/_/api/v1/pages"))
            .bearer_auth(&token)
            .json(&json!({"slug":slug,"markdown":""}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
        let r = c
            .post(format!(
                "{url}/_/api/v1/pages/by-slug/{slug}/verify-password"
            ))
            .header("x-forwarded-for", "203.0.113.1, 2001:db8::1")
            .json(&json!({"password":"wrong"}))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), if n == 10 { 429 } else { 400 });
    }
    for endpoint in [&login, &verify] {
        for n in 0..11 {
            let body = if endpoint == &login {
                json!({"username":"absent","password":"wrong"})
            } else {
                json!({"password":"wrong"})
            };
            let r = c
                .post(endpoint)
                .header("x-pagelet-csrf", "1")
                .header("x-forwarded-for", format!("198.51.100.{n}"))
                .json(&body)
                .send()
                .await
                .unwrap();
            let limit = if endpoint == &login { 10 } else { 9 };
            assert_eq!(r.status(), if n >= limit { 429 } else { 401 });
        }
    }
    for chain in [
        "unknown",
        "192.0.2.1,",
        "bad, 192.0.2.1",
        "192.0.2.1:123",
        "[::1]",
    ] {
        assert_eq!(
            c.post(&login)
                .header("x-pagelet-csrf", "1")
                .header("x-forwarded-for", chain)
                .json(&json!({"username":"fresh","password":"wrong"}))
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
    }
    server.abort();
}

#[test]
fn deployment_rejects_invalid_configuration() {
    for url in [
        "pages.example",
        "ftp://pages.example",
        "https://user@pages.example",
    ] {
        assert!(Deployment::new(Some(url), []).is_err());
    }
    for proxy in ["0.0.0.0/33", "::/129", "example.com", "127.0.0.1/-1"] {
        assert!(Deployment::new(None, [proxy]).is_err());
    }
}

#[test]
fn cli_deployment_flags_override_environment_and_reject_invalid_values() {
    let dir = tempfile::tempdir().unwrap();
    AppState::open(dir.path())
        .unwrap()
        .initialize("admin", "a long password")
        .unwrap();
    let command = || {
        let mut c = std::process::Command::new(env!("CARGO_BIN_EXE_pagelet"));
        c.env("PAGELET_DATA_DIR", dir.path())
            .env("PAGELET_LISTEN", "invalid listen address")
            .arg("serve");
        c
    };
    for (name, value) in [
        ("PAGELET_EXTERNAL_URL", "ftp://example.com"),
        ("PAGELET_TRUSTED_PROXIES", "127.0.0.1/33"),
    ] {
        let output = command().env(name, value).output().unwrap();
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("Could not bind"));
    }
    let output = command()
        .env("PAGELET_EXTERNAL_URL", "invalid")
        .env("PAGELET_TRUSTED_PROXIES", "invalid")
        .args([
            "--external-url",
            "https://pages.example",
            "--trusted-proxy",
            "127.0.0.1,::1",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Could not bind"));
}
