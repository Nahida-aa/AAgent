use std::sync::atomic::AtomicU64;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Child;

pub struct McpClient {
    id_counter: AtomicU64,
    stdin: tokio::sync::Mutex<tokio::process::ChildStdin>,
    stdout: tokio::sync::Mutex<BufReader<tokio::process::ChildStdout>>,
    _stderr: tokio::task::JoinHandle<()>,
    process: Option<Child>,
}

impl McpClient {
    pub async fn connect(command: &str, args: &[String]) -> anyhow::Result<Self> {
        use tokio::process::Command;

        let mut child = Command::new(command)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture stderr"))?;

        let stderr_handle = tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            while reader.read_line(&mut line).await.unwrap_or(0) > 0 {
                tracing::debug!("[mcp-stderr] {}", line.trim());
                line.clear();
            }
        });

        let mut client = Self {
            id_counter: AtomicU64::new(1),
            stdin: tokio::sync::Mutex::new(stdin),
            stdout: tokio::sync::Mutex::new(BufReader::new(stdout)),
            _stderr: stderr_handle,
            process: Some(child),
        };

        client.initialize().await?;
        Ok(client)
    }

    async fn initialize(&mut self) -> anyhow::Result<()> {
        let result: Value = self
            .send_request(
                "initialize",
                serde_json::json!({
                    "protocolVersion": "2025-03-26",
                    "capabilities": {},
                    "clientInfo": { "name": "AAgent", "version": "0.1.0" },
                }),
            )
            .await?;

        tracing::debug!("MCP init result: {result}");

        self.send_notification("notifications/initialized", serde_json::json!({}))
            .await?;

        Ok(())
    }

    pub async fn list_tools(&self) -> anyhow::Result<Vec<McpToolDef>> {
        let result: Value = self
            .send_request("tools/list", serde_json::json!({}))
            .await?;
        let tools = result["tools"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("MCP tools/list returned no tools array"))?
            .iter()
            .map(|t| McpToolDef {
                name: t["name"].as_str().unwrap_or("").to_owned(),
                description: t["description"].as_str().unwrap_or("").to_owned(),
                input_schema: t.get("inputSchema").cloned().unwrap_or(Value::Null),
            })
            .collect();
        Ok(tools)
    }

    pub async fn call_tool(&self, name: &str, arguments: Value) -> anyhow::Result<McpToolResult> {
        let result: Value = self
            .send_request(
                "tools/call",
                serde_json::json!({
                    "name": name,
                    "arguments": arguments,
                }),
            )
            .await?;

        let content = result["content"].as_array().cloned().unwrap_or_default();
        let text = content
            .iter()
            .filter_map(|c| c["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");

        Ok(McpToolResult {
            content: text,
            is_error: result["isError"].as_bool().unwrap_or(false),
        })
    }

    async fn send_request(&self, method: &str, params: Value) -> anyhow::Result<Value> {
        let id = self
            .id_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        {
            let mut stdin = self.stdin.lock().await;
            let line = serde_json::to_string(&request)?;
            stdin.write_all(line.as_bytes()).await?;
            stdin.write_all(b"\n").await?;
            stdin.flush().await?;
        }

        let mut stdout = self.stdout.lock().await;
        let mut line = String::new();
        loop {
            line.clear();
            let n = stdout.read_line(&mut line).await?;
            if n == 0 {
                anyhow::bail!("MCP server closed connection");
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let msg: Value = serde_json::from_str(trimmed)?;
            if msg.get("id").and_then(|v| v.as_u64()) == Some(id) {
                if let Some(error) = msg.get("error") {
                    anyhow::bail!(
                        "MCP error (id={id}): {}",
                        error["message"].as_str().unwrap_or("unknown")
                    );
                }
                return Ok(msg["result"].clone());
            }
        }
    }

    async fn send_notification(&self, method: &str, params: Value) -> anyhow::Result<()> {
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        let mut stdin = self.stdin.lock().await;
        let line = serde_json::to_string(&notification)?;
        stdin.write_all(line.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
        Ok(())
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.start_kill();
        }
    }
}

#[derive(Debug, Clone)]
pub struct McpToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone)]
pub struct McpToolResult {
    pub content: String,
    pub is_error: bool,
}
