pub mod transport;

use crate::config::McpServerConfig;
use crate::provider::ToolDefinition;
use std::collections::HashMap;
use transport::{McpTool, McpTransport};

pub struct McpManager {
    servers: HashMap<String, McpTransport>,
    tool_to_server: HashMap<String, String>,
}

impl McpManager {
    pub fn connect(servers_config: &HashMap<String, McpServerConfig>) -> Result<Self, String> {
        let mut servers = HashMap::new();

        for (name, config) in servers_config {
            let transport = McpTransport::spawn(&config.command, &config.args, &config.env)?;

            let init_result = transport.initialize()?;
            if let Some(protocol_version) = init_result.get("protocolVersion").and_then(|v| v.as_str()) {
                if protocol_version != "2024-11-05" {
                    return Err(format!(
                        "MCP server '{}' returned protocol version '{}', expected '2024-11-05'",
                        name, protocol_version
                    ));
                }
            }

            transport.send_notification("notifications/initialized", None)?;

            servers.insert(name.clone(), transport);
        }

        Ok(Self {
            servers,
            tool_to_server: HashMap::new(),
        })
    }

    pub fn discover_tools(&mut self) -> Result<Vec<ToolDefinition>, String> {
        let mut all_tools = Vec::new();
        self.tool_to_server.clear();

        for (server_name, transport) in &self.servers {
            let mcp_tools: Vec<McpTool> = transport.list_tools()?;
            for tool in mcp_tools {
                self.tool_to_server
                    .insert(tool.name.clone(), server_name.clone());
                all_tools.push(ToolDefinition {
                    name: tool.name.clone(),
                    description: tool.description.unwrap_or_default(),
                    parameters: tool.input_schema,
                });
            }
        }

        Ok(all_tools)
    }

    pub fn call_tool(&self, tool_name: &str, arguments: serde_json::Value) -> Result<String, String> {
        let server_name = self
            .tool_to_server
            .get(tool_name)
            .ok_or_else(|| format!("No MCP server found for tool '{}'", tool_name))?;

        let transport = self
            .servers
            .get(server_name)
            .ok_or_else(|| format!("MCP server '{}' not found", server_name))?;

        transport.call_tool(tool_name, arguments)
    }
}
