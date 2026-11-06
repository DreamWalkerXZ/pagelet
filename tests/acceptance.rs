use reqwest::{Client, Method};
use serde_json::{Value, json};
use std::{
    io::Write,
    path::Path,
    process::{Child, Command, Stdio},
    time::Duration,
};

struct Server {
    process: Child,
    url: String,
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}
async fn start(directory: &Path) -> Server {
    let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = socket.local_addr().unwrap();
    drop(socket);
    let process = Command::new(env!("CARGO_BIN_EXE_pagelet"))
        .env("PAGELET_DATA_DIR", directory)
        .env("PAGELET_LISTEN", address.to_string())
        .arg("serve")
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let server = Server {
        process,
        url: format!("http://{address}"),
    };
    for _ in 0..100 {
        if Client::new()
            .get(format!("{}/_/openapi.json", server.url))
            .send()
            .await
            .is_ok()
        {
            return server;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("executable did not start");
}
fn cli(directory: &Path, command: &str, input: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pagelet"))
        .args(["--data-dir", directory.to_str().unwrap(), command])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success());
    String::from_utf8(result.stdout)
        .unwrap()
        .lines()
        .find_map(|line| line.strip_prefix("API token: "))
        .unwrap()
        .to_owned()
}

fn references(document: &Value, node: &Value) {
    match node {
        Value::Object(fields) => {
            if let Some(reference) = fields.get("$ref") {
                let reference = reference.as_str().unwrap();
                assert!(
                    document
                        .pointer(reference.strip_prefix('#').unwrap())
                        .is_some(),
                    "unresolved {reference}"
                );
            }
            for value in fields.values() {
                references(document, value);
            }
        }
        Value::Array(items) => {
            for item in items {
                references(document, item);
            }
        }
        _ => (),
    }
}

// Check the structural schema vocabulary used by this contract, without a second schema engine.
fn schema(document: &Value, definition: &Value, value: &Value) {
    if let Some(reference) = definition["$ref"].as_str() {
        return schema(
            document,
            document
                .pointer(reference.strip_prefix('#').unwrap())
                .expect("resolved reference"),
            value,
        );
    }
    if let Some(expected) = definition.get("const") {
        assert_eq!(value, expected);
    }
    if let Some(options) = definition["enum"].as_array() {
        assert!(options.contains(value));
    }
    match definition["type"].as_str() {
        Some("object") => {
            let object = value.as_object().expect("object response");
            if let Some(required) = definition["required"].as_array() {
                for key in required {
                    assert!(object.contains_key(key.as_str().unwrap()), "missing {key}");
                }
            }
            for (key, value) in object {
                if let Some(property) = definition["properties"].get(key) {
                    schema(document, property, value);
                } else {
                    assert_ne!(definition["additionalProperties"], false);
                }
            }
        }
        Some("array") => {
            for item in value.as_array().expect("array response") {
                schema(document, &definition["items"], item);
            }
        }
        Some("integer") => {
            assert!(value.is_i64() || value.is_u64());
            if let Some(min) = definition["minimum"].as_i64() {
                assert!(value.as_i64().unwrap() >= min);
            }
        }
        Some("string") => {
            assert!(value.is_string());
        }
        None => (),
        other => panic!("unsupported contract type {other:?}"),
    }
}
#[allow(clippy::too_many_arguments)]
async fn request(
    server: &Server,
    contract: &Value,
    method: &str,
    path: &str,
    template: &str,
    token: Option<&str>,
    body: Option<Value>,
    expected: u16,
) -> Value {
    let operation = &contract["paths"][template][method];
    assert!(operation.is_object(), "undocumented {method} {template}");
    let mut request = Client::new().request(
        Method::from_bytes(method.to_uppercase().as_bytes()).unwrap(),
        format!("{}{path}", server.url),
    );
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    if let Some(body) = body {
        schema(
            contract,
            &operation["requestBody"]["content"]["application/json"]["schema"],
            &body,
        );
        request = request.json(&body);
    }
    let response = request.send().await.unwrap();
    assert_eq!(response.status(), expected, "{method} {path}");
    let definition = &operation["responses"][expected.to_string()];
    assert!(
        definition.is_object(),
        "undocumented status {expected} for {template}"
    );
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let text = response.text().await.unwrap();
    if text.is_empty() {
        return Value::Null;
    }
    if content_type.starts_with("application/json") {
        let value = serde_json::from_str(&text).unwrap();
        if let Some(definition) = definition.pointer("/content/application~1json/schema") {
            schema(contract, definition, &value);
        }
        value
    } else {
        assert!(
            definition["content"]
                .as_object()
                .is_some_and(|types| types.keys().any(|t| content_type.starts_with(t))),
            "undocumented content type {content_type}"
        );
        Value::String(text)
    }
}

#[tokio::test]
async fn executable_api_lifecycle_survives_stopped_directory_backup_and_restore() {
    let directory = tempfile::tempdir().unwrap();
    let admin = cli(
        directory.path(),
        "init",
        "admin\na long password\na long password\n",
    );
    let server = start(directory.path()).await;
    let contract: Value = Client::new()
        .get(format!("{}/_/openapi.json", server.url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(contract["openapi"], "3.1.0");
    references(&contract, &contract);
    assert_eq!(
        contract["components"]["securitySchemes"]["bearerAuth"]["scheme"],
        "bearer"
    );
    assert_eq!(
        contract["components"]["securitySchemes"]["sessionAuth"]["in"],
        "cookie"
    );
    assert_eq!(
        contract["components"]["securitySchemes"]["pageGrant"]["name"],
        "X-Pagelet-Grant"
    );
    assert_eq!(
        contract["paths"]["/_/api/v1/me"]["get"]["security"],
        json!([{"bearerAuth":[]},{"sessionAuth":[]}])
    );
    let login = Client::new()
        .post(format!("{}/_/api/v1/login", server.url))
        .header("X-Pagelet-CSRF", "1")
        .json(&json!({"username":"admin","password":"a long password"}))
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
        .to_owned();
    schema(
        &contract,
        &contract["paths"]["/_/api/v1/login"]["post"]["responses"]["200"]["content"]["application/json"]
            ["schema"],
        &login.json::<Value>().await.unwrap(),
    );
    let session = Client::new()
        .get(format!("{}/_/api/v1/me", server.url))
        .header("Cookie", cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(session.status(), 200);
    schema(
        &contract,
        &contract["components"]["schemas"]["Account"],
        &session.json::<Value>().await.unwrap(),
    );
    let user = request(
        &server,
        &contract,
        "post",
        "/_/api/v1/users",
        "/_/api/v1/users",
        Some(&admin),
        Some(json!({"username":"alice","password":"a long password","role":"user"})),
        201,
    )
    .await;
    let token = user["token"].as_str().unwrap();
    let page = request(
        &server,
        &contract,
        "post",
        "/_/api/v1/pages",
        "/_/api/v1/pages",
        Some(token),
        Some(json!({"slug":"acceptance","markdown":"# Durable"})),
        201,
    )
    .await;
    let id = page["id"].as_i64().unwrap();
    let path = format!("/_/api/v1/pages/{id}");
    assert_eq!(
        request(
            &server,
            &contract,
            "get",
            "/_/raw/acceptance",
            "/_/raw/{slug}",
            None,
            None,
            200
        )
        .await,
        "# Durable"
    );
    request(
        &server,
        &contract,
        "patch",
        &path,
        "/_/api/v1/pages/{id}",
        Some(token),
        Some(json!({"version":1,"access_password":"secret"})),
        200,
    )
    .await;
    request(
        &server,
        &contract,
        "patch",
        &path,
        "/_/api/v1/pages/{id}",
        Some(token),
        Some(json!({"version":1,"markdown":"stale"})),
        409,
    )
    .await;
    assert_eq!(
        request(
            &server,
            &contract,
            "get",
            "/_/api/v1/pages/by-slug/acceptance",
            "/_/api/v1/pages/by-slug/{slug}",
            None,
            None,
            401
        )
        .await,
        json!({"error":"password_required"})
    );
    let grant = request(
        &server,
        &contract,
        "post",
        "/_/api/v1/pages/by-slug/acceptance/verify-password",
        "/_/api/v1/pages/by-slug/{slug}/verify-password",
        None,
        Some(json!({"password":"secret"})),
        200,
    )
    .await;
    let read = Client::new()
        .get(format!("{}/_/raw/acceptance", server.url))
        .header("X-Pagelet-Grant", grant["grant"].as_str().unwrap())
        .send()
        .await
        .unwrap();
    assert_eq!(read.status(), 200);
    assert_eq!(read.headers()["cache-control"], "no-store");
    assert_eq!(read.text().await.unwrap(), "# Durable");
    let rotated = request(
        &server,
        &contract,
        "post",
        "/_/api/v1/me/token",
        "/_/api/v1/me/token",
        Some(token),
        None,
        200,
    )
    .await;
    request(
        &server,
        &contract,
        "get",
        "/_/api/v1/me",
        "/_/api/v1/me",
        Some(token),
        None,
        401,
    )
    .await;
    let token = rotated["token"].as_str().unwrap();
    request(
        &server,
        &contract,
        "get",
        "/_/api/v1/me",
        "/_/api/v1/me",
        Some(token),
        None,
        200,
    )
    .await;
    for _ in 0..9 {
        request(
            &server,
            &contract,
            "post",
            "/_/api/v1/pages/by-slug/acceptance/verify-password",
            "/_/api/v1/pages/by-slug/{slug}/verify-password",
            None,
            Some(json!({"password":"wrong"})),
            401,
        )
        .await;
    }
    request(
        &server,
        &contract,
        "post",
        "/_/api/v1/pages/by-slug/acceptance/verify-password",
        "/_/api/v1/pages/by-slug/{slug}/verify-password",
        None,
        Some(json!({"password":"secret"})),
        429,
    )
    .await;
    drop(server);
    let backup = tempfile::tempdir().unwrap();
    for entry in std::fs::read_dir(directory.path()).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), backup.path().join(entry.file_name())).unwrap();
    }
    let server = start(backup.path()).await;
    let persisted = request(
        &server,
        &contract,
        "get",
        &path,
        "/_/api/v1/pages/{id}",
        Some(token),
        None,
        200,
    )
    .await;
    assert_eq!(persisted["markdown"], "# Durable");
    assert_eq!(persisted["version"], 2);
    request(
        &server,
        &contract,
        "post",
        &format!("{path}/owner"),
        "/_/api/v1/pages/{id}/owner",
        Some(&admin),
        Some(json!({"version":2,"owner_id":1})),
        200,
    )
    .await;
    request(
        &server,
        &contract,
        "get",
        &path,
        "/_/api/v1/pages/{id}",
        Some(token),
        None,
        403,
    )
    .await;
    request(
        &server,
        &contract,
        "delete",
        &format!("/_/api/v1/users/{}", user["user"]["id"]),
        "/_/api/v1/users/{id}",
        Some(&admin),
        None,
        204,
    )
    .await;
    request(
        &server,
        &contract,
        "delete",
        &path,
        "/_/api/v1/pages/{id}",
        Some(&admin),
        Some(json!({"version":3})),
        204,
    )
    .await;
    request(
        &server,
        &contract,
        "get",
        "/_/raw/acceptance",
        "/_/raw/{slug}",
        None,
        None,
        404,
    )
    .await;
}
