use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
}

pub struct McpTransport {
    child: Child,
    stdin: Mutex<ChildStdin>,
    stdout: Mutex<BufReader<ChildStdout>>,
    request_id: AtomicU64,
}

impl McpTransport {
    pub fn spawn(
        command: &str,
        args: &[String],
        env: &std::collections::HashMap<String, String>,
    ) -> Result<Self, String> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (k, v) in env {
            cmd.env(k, v);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn MCP server '{}': {}", command, e))?;
        let stdin = child
            .stdin
            .take()
            .ok_or("Failed to get stdin of MCP server")?;
        let stdout = child
            .stdout
            .take()
            .ok_or("Failed to get stdout of MCP server")?;

        Ok(Self {
            child,
            stdin: Mutex::new(stdin),
            stdout: Mutex::new(BufReader::new(stdout)),
            request_id: AtomicU64::new(0),
        })
    }

    pub fn initialize(&self) -> Result<Value, String> {
        let params = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "agent-runner",
                "version": "0.1.0"
            }
        });
        self.call("initialize", Some(params))
    }

    pub fn list_tools(&self) -> Result<Vec<McpTool>, String> {
        let result = self.call("tools/list", None)?;
        let tools: Vec<McpTool> =
            serde_json::from_value(result.get("tools").cloned().unwrap_or(Value::Array(vec![])))
                .map_err(|e| format!("Failed to parse tools/list response: {}", e))?;
        Ok(tools)
    }

    pub fn call_tool(&self, name: &str, arguments: Value) -> Result<String, String> {
        let params = serde_json::json!({
            "name": name,
            "arguments": arguments,
        });
        let result = self.call("tools/call", Some(params))?;
        let content = result
            .get("content")
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| item.get("text").and_then(|t| t.as_str()).map(String::from))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_else(|| result.to_string());
        Ok(content)
    }

    pub fn call(&self, method: &str, params: Option<Value>) -> Result<Value, String> {
        let id = self.request_id.fetch_add(1, Ordering::Relaxed);
        let mut request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
        });
        if let Some(p) = params {
            request["params"] = p;
        }

        let body = serde_json::to_string(&request)
            .map_err(|e| format!("Failed to serialize request: {}", e))?;
        let message = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);

        {
            let mut stdin = self
                .stdin
                .lock()
                .map_err(|e| format!("stdin lock: {}", e))?;
            stdin
                .write_all(message.as_bytes())
                .map_err(|e| format!("Failed to write to MCP server: {}", e))?;
            stdin
                .flush()
                .map_err(|e| format!("Failed to flush stdin: {}", e))?;
        }

        let result = {
            let mut stdout = self
                .stdout
                .lock()
                .map_err(|e| format!("stdout lock: {}", e))?;
            let mut header = String::new();
            loop {
                let mut byte = [0u8; 1];
                stdout
                    .read_exact(&mut byte)
                    .map_err(|e| format!("Failed to read header from MCP server: {}", e))?;
                header.push(byte[0] as char);
                if header.ends_with("\r\n\r\n") {
                    break;
                }
                if header.len() > 1024 {
                    return Err("Header too long reading MCP response".into());
                }
            }

            let content_length = header
                .lines()
                .find(|l| l.starts_with("Content-Length:"))
                .and_then(|l| {
                    l.trim_start_matches("Content-Length:")
                        .trim()
                        .parse::<usize>()
                        .ok()
                })
                .ok_or_else(|| {
                    format!("Missing Content-Length in MCP response header: {}", header)
                })?;

            let mut body_buf = vec![0u8; content_length];
            stdout
                .read_exact(&mut body_buf)
                .map_err(|e| format!("Failed to read MCP response body: {}", e))?;

            let response: Value = serde_json::from_slice(&body_buf)
                .map_err(|e| format!("Failed to parse MCP response: {}", e))?;

            if let Some(error) = response.get("error") {
                let msg = error
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("Unknown error");
                let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
                return Err(format!("MCP error (code {}): {}", code, msg));
            }

            response
                .get("result")
                .cloned()
                .ok_or_else(|| "Missing result in MCP response".into())
        };
        result
    }

    pub fn send_notification(&self, method: &str, params: Option<Value>) -> Result<(), String> {
        let mut notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
        });
        if let Some(p) = params {
            notification["params"] = p;
        }

        let body = serde_json::to_string(&notification)
            .map_err(|e| format!("Failed to serialize notification: {}", e))?;
        let message = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);

        let mut stdin = self
            .stdin
            .lock()
            .map_err(|e| format!("stdin lock: {}", e))?;
        stdin
            .write_all(message.as_bytes())
            .map_err(|e| format!("Failed to write notification: {}", e))?;
        stdin
            .flush()
            .map_err(|e| format!("Failed to flush stdin: {}", e))?;
        Ok(())
    }
}

impl Drop for McpTransport {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
