use reqwest::{Client, RequestBuilder};
use serde_json::{Value, json};
use std::{
    process::{Child, Command},
    time::Duration,
};

pub struct TestServer {
    _directory: tempfile::TempDir,
    process: Child,
    pub client: Client,
    pub base_url: String,
    pub admin: String,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

impl TestServer {
    pub async fn start(enabled: bool, external_url: Option<&str>) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let admin = pagelet::AppState::open(directory.path())
            .unwrap()
            .initialize("admin", "a long password")
            .unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let mut command = Command::new(env!("CARGO_BIN_EXE_pagelet"));
        command
            .args([
                "--data-dir",
                directory.path().to_str().unwrap(),
                "serve",
                "--listen",
                &address.to_string(),
            ])
            .env_remove("PAGELET_MCP")
            .env_remove("PAGELET_EXTERNAL_URL");
        if enabled {
            command.arg("--mcp");
        }
        if let Some(external_url) = external_url {
            command.args(["--external-url", external_url]);
        }
        let mut server = Self {
            process: command.spawn().unwrap(),
            _directory: directory,
            client: Client::new(),
            base_url: format!("http://{address}"),
            admin,
        };
        for _ in 0..100 {
            if server.client.get(&server.base_url).send().await.is_ok() {
                return server;
            }
            assert!(
                server.process.try_wait().unwrap().is_none(),
                "Pagelet exited before serving HTTP"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("Pagelet did not start");
    }

    pub async fn user(&self, username: &str) -> String {
        self.client
            .post(format!("{}/_/api/v1/users", self.base_url))
            .bearer_auth(&self.admin)
            .json(&json!({"username": username, "password": "a long password", "role": "user"}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()["token"]
            .as_str()
            .unwrap()
            .into()
    }

    pub async fn publish(&self, token: &str, slug: &str, markdown: &str) -> Value {
        self.client
            .post(format!("{}/_/api/v1/pages", self.base_url))
            .bearer_auth(token)
            .json(&json!({"slug": slug, "markdown": markdown}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    pub async fn connect(&self, token: &str) -> McpClient {
        let client = McpClient {
            client: self.client.clone(),
            url: format!("{}/_/mcp", self.base_url),
            token: token.into(),
        };
        let init = client
            .rpc(
                "initialize",
                json!({
                    "protocolVersion": "2025-11-25",
                    "capabilities": {},
                    "clientInfo": {"name":"pagelet-test", "version":"1"}
                }),
            )
            .await;
        assert_eq!(init["protocolVersion"], "2025-11-25");
        assert!(init["capabilities"]["tools"].is_object());
        assert_eq!(
            client
                .request(json!({"jsonrpc":"2.0", "method":"notifications/initialized"}))
                .send()
                .await
                .unwrap()
                .status(),
            202
        );
        client
    }

    pub fn mcp_request(&self, payload: Value) -> RequestBuilder {
        self.client
            .post(format!("{}/_/mcp", self.base_url))
            .header("Accept", "application/json, text/event-stream")
            .header("MCP-Protocol-Version", "2025-11-25")
            .json(&payload)
    }
}

pub struct McpClient {
    client: Client,
    url: String,
    token: String,
}

impl McpClient {
    fn request(&self, payload: Value) -> RequestBuilder {
        self.client
            .post(&self.url)
            .header("Accept", "application/json, text/event-stream")
            .header("MCP-Protocol-Version", "2025-11-25")
            .bearer_auth(&self.token)
            .json(&payload)
    }

    async fn rpc(&self, method: &str, params: Value) -> Value {
        let response: Value = self
            .request(json!({"jsonrpc":"2.0", "id":1, "method":method, "params":params}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(response["id"], 1);
        assert!(response.get("error").is_none(), "{response}");
        response["result"].clone()
    }

    pub async fn tools(&self) -> Value {
        self.rpc("tools/list", json!({})).await["tools"].clone()
    }

    pub async fn call(&self, name: &str, arguments: Value) -> Value {
        self.rpc("tools/call", json!({"name": name, "arguments": arguments}))
            .await
    }
}
