use std::{
    io::Write,
    process::{Command, Stdio},
    time::Duration,
};

#[tokio::test]
async fn initialized_account_survives_restart_and_authenticates() {
    let directory = tempfile::tempdir().unwrap();
    let binary = env!("CARGO_BIN_EXE_pagelet");
    let refusal = Command::new(binary)
        .args(["--data-dir", directory.path().to_str().unwrap(), "serve"])
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
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let mut server = Command::new(binary)
        .env("PAGELET_DATA_DIR", directory.path())
        .env("PAGELET_LISTEN", address.to_string())
        .arg("serve")
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let client = reqwest::Client::new();
    let url = format!("http://{address}/_/api/v1/me");
    for _ in 0..100 {
        if client.get(&url).send().await.is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
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
    server.kill().unwrap();
    server.wait().unwrap();
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
    database.pragma_update(None, "user_version", 2).unwrap();
    drop(database);
    let result = Command::new(env!("CARGO_BIN_EXE_pagelet"))
        .args(["--data-dir", directory.path().to_str().unwrap(), "init"])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("newer"));
}
