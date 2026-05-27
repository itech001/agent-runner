use async_trait::async_trait;
use serde_json::{json, Value};
use std::time::Duration;

use super::{Message, Provider, ProviderError, ProviderResponse, ToolDefinition};

pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    max_tokens: u32,
    temperature: f32,
}

impl AnthropicProvider {
    pub fn new(
        api_key: String,
        model: String,
        max_tokens: u32,
        temperature: f32,
    ) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
            api_key,
            model,
            max_tokens,
            temperature,
        }
    }

    fn convert_messages(&self, messages: &[Message]) -> (Option<Value>, Vec<Value>) {
        let mut system_content = None;
        let mut converted = Vec::new();

        for msg in messages {
            match msg.role.as_str() {
                "system" => {
                    system_content = msg.content.as_ref().map(|c| json!(c));
                }
                "user" => {
                    converted.push(json!({
                        "role": "user",
                        "content": msg.content.as_deref().unwrap_or(""),
                    }));
                }
                "assistant" => {
                    let mut content_blocks: Vec<Value> = Vec::new();
                    if let Some(ref text) = msg.content {
                        content_blocks.push(json!({
                            "type": "text",
                            "text": text,
                        }));
                    }
                    if let Some(ref tool_calls) = msg.tool_calls {
                        for tc in tool_calls {
                            content_blocks.push(json!({
                                "type": "tool_use",
                                "id": tc.id,
                                "name": tc.name,
                                "input": tc.arguments,
                            }));
                        }
                    }
                    converted.push(json!({
                        "role": "assistant",
                        "content": content_blocks,
                    }));
                }
                "tool" => {
                    let tool_call_id = msg.tool_call_id.as_deref().unwrap_or("");
                    let last = converted.last_mut();
                    if let Some(last_msg) = last {
                        if last_msg["role"].as_str() == Some("user") {
                            if let Some(arr) = last_msg["content"].as_array_mut() {
                                arr.push(json!({
                                    "type": "tool_result",
                                    "tool_use_id": tool_call_id,
                                    "content": msg.content.as_deref().unwrap_or(""),
                                }));
                                continue;
                            }
                        }
                    }
                    converted.push(json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": tool_call_id,
                            "content": msg.content.as_deref().unwrap_or(""),
                        }],
                    }));
                }
                _ => {}
            }
        }

        (system_content, converted)
    }

    fn convert_tools(&self, tools: &[ToolDefinition]) -> Vec<Value> {
        tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.parameters,
                })
            })
            .collect()
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<ProviderResponse, ProviderError> {
        let (system, anthropic_messages) = self.convert_messages(messages);

        let mut body = json!({
            "model": self.model,
            "messages": anthropic_messages,
            "max_tokens": self.max_tokens,
            "temperature": self.temperature,
        });
        if let Some(sys) = system {
            body["system"] = sys;
        }
        if !tools.is_empty() {
            body["tools"] = json!(self.convert_tools(tools));
        }

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        let status = response.status();
        if status.as_u16() == 429 {
            return Err(ProviderError::RateLimited);
        }
        if status.as_u16() == 408 {
            return Err(ProviderError::Timeout);
        }
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(ProviderError::Api(format!("{}: {}", status, text)));
        }

        let resp: Value = response
            .json()
            .await
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        let usage_input = resp["usage"]["input_tokens"]
            .as_u64()
            .unwrap_or(0) as u32;
        let usage_output = resp["usage"]["output_tokens"]
            .as_u64()
            .unwrap_or(0) as u32;

        let mut content_text = None;
        let mut tool_calls = Vec::new();
        let mut reasoning = None;
        let mut done = false;

        if let Some(blocks) = resp["content"].as_array() {
            for block in blocks {
                match block["type"].as_str() {
                    Some("text") => {
                        if let Some(text) = block["text"].as_str() {
                            content_text = Some(text.to_string());
                        }
                    }
                    Some("thinking") => {
                        if let Some(text) = block["thinking"].as_str() {
                            reasoning = Some(text.to_string());
                        }
                    }
                    Some("tool_use") => {
                        let id = block["id"].as_str().unwrap_or_default().to_string();
                        let name = block["name"].as_str().unwrap_or_default().to_string();
                        let input = block["input"].clone();
                        if name == "task_done" {
                            done = true;
                        }
                        tool_calls.push(super::ToolCall {
                            id,
                            name,
                            arguments: input,
                        });
                    }
                    _ => {}
                }
            }
        }

        Ok(ProviderResponse {
            content: content_text,
            tool_calls,
            usage: super::Usage {
                input_tokens: usage_input,
                output_tokens: usage_output,
            },
            reasoning,
            done,
        })
    }
}
