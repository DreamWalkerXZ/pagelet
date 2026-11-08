use std::{
    io::{Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    time::Duration,
};

struct TestServer(Child);

impl Drop for TestServer {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(None)) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn has_event(logs: &str, fields: &[&str]) -> bool {
    logs.lines().any(|line| {
        fields
            .iter()
            .all(|field| line.split_whitespace().any(|part| part == *field))
    })
}

async fn start_server(
    directory: &Path,
    client: &reqwest::Client,
    filter: Option<&str>,
    bootstrap: &[(&str, &str)],
) -> (TestServer, String) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let mut command = Command::new(env!("CARGO_BIN_EXE_pagelet"));
    command
        .env_remove("RUST_LOG")
        .env_remove("PAGELET_ADMIN_USERNAME")
        .env_remove("PAGELET_ADMIN_PASSWORD")
        .envs(bootstrap.iter().copied());
    if let Some(filter) = filter {
        command.env("RUST_LOG", filter);
    }
    let server = TestServer(
        command
            .env("PAGELET_DATA_DIR", directory)
            .env("PAGELET_LISTEN", address.to_string())
            .arg("serve")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let url = format!("http://{address}/_/api/v1/me");
    for _ in 0..100 {
        if client.get(&url).send().await.is_ok() {
            return (server, address.to_string());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("server did not start");
}

async fn stop_server(mut server: TestServer) -> String {
    let server = &mut server.0;
    let signaled = Command::new("kill")
        .args(["-s", "INT", &server.id().to_string()])
        .status()
        .unwrap();
    assert!(signaled.success());
    for _ in 0..100 {
        if server.try_wait().unwrap().is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    if server.try_wait().unwrap().is_none() {
        server.kill().unwrap();
    }
    assert!(
        server.wait().unwrap().success(),
        "server did not shut down gracefully"
    );
    let mut logs = String::new();
    server
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut logs)
        .unwrap();
    let mut stdout = String::new();
    server
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut stdout)
        .unwrap();
    assert!(stdout.is_empty(), "serve printed to stdout");
    logs
}

#[tokio::test]
async fn initialized_account_survives_restart_and_authenticates() {
    let directory = tempfile::tempdir().unwrap();
    let binary = env!("CARGO_BIN_EXE_pagelet");
    let refusal = Command::new(binary)
        .args(["--data-dir", directory.path().to_str().unwrap(), "serve"])
        .env_remove("PAGELET_ADMIN_USERNAME")
        .env_remove("PAGELET_ADMIN_PASSWORD")
        .output()
        .unwrap();
    assert!(!refusal.status.success());
    let mut init = Command::new(binary)
        .args(["--data-dir", directory.path().to_str().unwrap(), "init"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    init.stdin
        .take()
        .unwrap()
        .write_all(b"admin\na long password\na long password\n")
        .unwrap();
    let initialized = init.wait_with_output().unwrap();
    assert!(initialized.status.success());
    let output = String::from_utf8(initialized.stdout).unwrap();
    let token = output
        .lines()
        .find_map(|line| line.strip_prefix("API token: "))
        .unwrap();
    let client = reqwest::Client::new();
    let (server, address) = start_server(directory.path(), &client, None, &[]).await;
    let url = format!("http://{address}/_/api/v1/me");
    let response = client.get(&url).bearer_auth(token).send().await.unwrap();
    let status = response.status();
    let body: serde_json::Value = response.json().await.unwrap();
    let unauthorized = client
        .get(&url)
        .bearer_auth("invalid")
        .send()
        .await
        .unwrap()
        .status();
    let openapi: serde_json::Value = client
        .get(format!("http://{address}/_/openapi.json"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let login_url = format!("http://{address}/_/api/v1/login");
    for (password, expected) in [("wrong password", 401), ("a long password", 200)] {
        let response = client
            .post(&login_url)
            .header("x-pagelet-csrf", "1")
            .json(&serde_json::json!({"username":"admin","password":password}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
    }
    let logs = stop_server(server).await;
    assert!(has_event(
        &logs,
        &["INFO", "event=listening", &format!("address={address}")]
    ));
    assert!(has_event(
        &logs,
        &["INFO", "event=login_succeeded", "username=admin"]
    ));
    assert!(has_event(
        &logs,
        &["WARN", "event=login_failed", "source=127.0.0.1"]
    ));
    assert!(has_event(
        &logs,
        &["INFO", "event=shutdown_requested", "signal=SIGINT"]
    ));
    assert!(has_event(&logs, &["INFO", "event=shutdown_completed"]));
    assert!(!logs.contains("event=request_completed"));
    for secret in [
        token,
        "a long password",
        "wrong password",
        "pagelet_session",
    ] {
        assert!(!logs.contains(secret));
    }
    assert_eq!(status, 200);
    assert_eq!(
        body,
        serde_json::json!({"id":1,"username":"admin","role":"administrator"})
    );
    assert_eq!(unauthorized, 401);
    assert!(openapi["paths"]["/_/api/v1/me"].is_object());
    let repeat = Command::new(binary)
        .args(["--data-dir", directory.path().to_str().unwrap(), "init"])
        .output()
        .unwrap();
    assert!(!repeat.status.success());
}

#[test]
fn initialization_rejects_invalid_credentials_without_creating_an_account() {
    for (username, password, confirmation) in [
        ("Admin", "a long password", "a long password"),
        ("ab", "a long password", "a long password"),
        ("admin--one", "a long password", "a long password"),
        ("admin", "short", "short"),
        ("admin", "a long password", "different password"),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let mut process = Command::new(env!("CARGO_BIN_EXE_pagelet"))
            .args(["--data-dir", directory.path().to_str().unwrap(), "init"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        process
            .stdin
            .take()
            .unwrap()
            .write_all(format!("{username}\n{password}\n{confirmation}\n").as_bytes())
            .unwrap();
        let result = process.wait_with_output().unwrap();
        assert!(!result.status.success());
        assert!(!String::from_utf8_lossy(&result.stdout).contains("API token:"));
        let result = Command::new(env!("CARGO_BIN_EXE_pagelet"))
            .args(["--data-dir", directory.path().to_str().unwrap(), "serve"])
            .env_remove("PAGELET_ADMIN_USERNAME")
            .env_remove("PAGELET_ADMIN_PASSWORD")
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("not initialized"));
    }
}

#[test]
fn newer_schema_is_rejected_before_initialization() {
    let directory = tempfile::tempdir().unwrap();
    let database = rusqlite::Connection::open(directory.path().join("pagelet.sqlite3")).unwrap();
    database.pragma_update(None, "user_version", 999).unwrap();
    drop(database);
    let result = Command::new(env!("CARGO_BIN_EXE_pagelet"))
        .args(["--data-dir", directory.path().to_str().unwrap(), "init"])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("newer"));
}

#[tokio::test]
async fn service_filters_logs_and_only_summarizes_safe_request_fields_at_debug() {
    let directory = tempfile::tempdir().unwrap();
    let state = pagelet::AppState::open(directory.path()).unwrap();
    let token = state.initialize("admin", "a long password").unwrap();
    drop(state);
    let client = reqwest::Client::new();

    for (filter, info, debug, invalid) in [
        ("pagelet=warn", false, false, false),
        ("pagelet=debug", true, true, false),
        ("pagelet=debug,pagelet[{username}]=off", true, true, false),
        ("pagelet=invalid-log-level-marker", true, false, true),
        ("", true, false, false),
    ] {
        let (server, address) = start_server(directory.path(), &client, Some(filter), &[]).await;
        let base = format!("http://{address}");
        let matched = client
            .get(format!(
                "{base}/_/api/v1/pages/by-slug/private-slug-marker?q=private-query-marker"
            ))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap();
        assert_eq!(matched.status(), 404);
        let unmatched = client
            .get(format!("{base}/no-matched-route-marker/extra"))
            .send()
            .await
            .unwrap();
        assert_eq!(unmatched.status(), 404);
        let rejected = client
            .post(format!("{base}/_/api/v1/login"))
            .header("cookie", "pagelet_session=private-cookie-marker")
            .body("private-body-marker")
            .send()
            .await
            .unwrap();
        assert_eq!(rejected.status(), 403);
        let failed = client
            .post(format!("{base}/_/api/v1/login"))
            .header("x-pagelet-csrf", "1")
            .json(&serde_json::json!({"username":"admin","password":"private-password-marker"}))
            .send()
            .await
            .unwrap();
        assert_eq!(failed.status(), 401);

        let logs = stop_server(server).await;
        assert_eq!(
            has_event(&logs, &["INFO", "event=listening"]),
            info,
            "filter {filter}"
        );
        assert_eq!(
            has_event(&logs, &["INFO", "event=shutdown_completed"]),
            info
        );
        assert!(has_event(
            &logs,
            &["WARN", "event=login_failed", "source=127.0.0.1"]
        ));
        assert_eq!(
            logs.matches("event=invalid_log_filter").count(),
            usize::from(invalid)
        );
        assert_eq!(
            has_event(&logs, &["WARN", "event=invalid_log_filter"]),
            invalid
        );
        assert!(!logs.contains("ERROR"));
        let summaries: Vec<_> = logs
            .lines()
            .filter(|line| line.contains("event=request_completed"))
            .collect();
        if debug {
            for (method, route, status) in [
                ("GET", "/_/api/v1/pages/by-slug/{slug}", 404),
                ("GET", "unmatched", 404),
                ("POST", "/_/api/v1/login", 403),
                ("POST", "/_/api/v1/login", 401),
            ] {
                let fields = [
                    format!("method={method}"),
                    format!("route={route}"),
                    format!("status={status}"),
                ];
                let matches: Vec<_> = summaries
                    .iter()
                    .filter(|line| {
                        fields
                            .iter()
                            .all(|field| line.split_whitespace().any(|part| part == field))
                    })
                    .collect();
                assert_eq!(
                    matches.len(),
                    1,
                    "expected one summary for {method} {route} {status}"
                );
                assert!(matches[0].contains("DEBUG"));
                assert!(!matches[0].contains("source="));
                let elapsed: f64 = matches[0]
                    .split_whitespace()
                    .find_map(|field| field.strip_prefix("elapsed_ms="))
                    .unwrap()
                    .parse()
                    .unwrap();
                assert!(elapsed.is_finite() && elapsed >= 0.0);
            }
        } else {
            assert!(summaries.is_empty(), "filter {filter}");
        }
        for secret in [
            token.as_str(),
            "private-slug-marker",
            "private-query-marker",
            "no-matched-route-marker",
            "private-cookie-marker",
            "private-body-marker",
            "private-password-marker",
            "invalid-log-level-marker",
        ] {
            assert!(!logs.contains(secret), "logs included a sensitive value");
        }
    }
}

#[test]
fn startup_reports_missing_bootstrap_variables_without_echoing_credentials() {
    for (present, missing) in [
        ("PAGELET_ADMIN_USERNAME", "PAGELET_ADMIN_PASSWORD"),
        ("PAGELET_ADMIN_PASSWORD", "PAGELET_ADMIN_USERNAME"),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_pagelet"))
            .arg("--data-dir")
            .arg(directory.path())
            .arg("serve")
            .env_remove(missing)
            .env(present, "private-value")
            .output()
            .unwrap();
        assert!(!result.status.success());
        let logs = String::from_utf8_lossy(&result.stderr);
        assert!(logs.contains(missing), "{logs}");
        assert!(!logs.contains("private-value"));
        assert!(result.stdout.is_empty());
    }
}

#[tokio::test]
async fn environment_bootstrap_is_private_and_only_runs_once() {
    let directory = tempfile::tempdir().unwrap();
    let client = reqwest::Client::new();
    for (first_start, username, password) in [
        (true, "bootstrap-admin", "bootstrap password"),
        (false, "Invalid username", "short"),
    ] {
        let (server, address) = start_server(
            directory.path(),
            &client,
            None,
            &[
                ("PAGELET_ADMIN_USERNAME", username),
                ("PAGELET_ADMIN_PASSWORD", password),
            ],
        )
        .await;
        let base = format!("http://{address}");
        let login = client
            .post(format!("{base}/_/api/v1/login"))
            .header("x-pagelet-csrf", "1")
            .json(&serde_json::json!({
                "username": "bootstrap-admin",
                "password": "bootstrap password",
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(login.status(), 200);
        if first_start {
            let cookie = login.headers()["set-cookie"].to_str().unwrap();
            let page = client
                .post(format!("{base}/_/api/v1/pages"))
                .header("cookie", cookie.split(';').next().unwrap())
                .header("x-pagelet-csrf", "1")
                .json(&serde_json::json!({"slug":"bootstrap","markdown":"# Preserved"}))
                .send()
                .await
                .unwrap();
            assert_eq!(page.status(), 201);
        }
        let raw = client
            .get(format!("{base}/_/raw/bootstrap"))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
        assert_eq!(raw.text().await.unwrap(), "# Preserved");

        let logs = stop_server(server).await;
        let event = [
            "INFO",
            "event=administrator_initialized",
            "source=environment",
        ];
        assert_eq!(has_event(&logs, &event), first_start);
        for line in logs.lines().filter(|line| line.contains(event[1])) {
            assert!(!line.contains(username));
        }
        assert!(!logs.contains("bootstrap password"));
        assert!(!logs.contains("API token"));
    }
}
