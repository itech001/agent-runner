use async_trait::async_trait;
use serde_json::{json, Value};
use std::time::Duration;

use super::{Message, Provider, ProviderError, ProviderResponse, ToolDefinition};

pub struct OpenAiProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
    max_tokens: u32,
    temperature: f32,
}

impl OpenAiProvider {
    pub fn new(
        api_key: String,
        model: String,
        base_url: Option<String>,
        max_tokens: u32,
        temperature: f32,
    ) -> Self {
        let base_url = base_url
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
            api_key,
            model,
            base_url,
            max_tokens,
            temperature,
        }
    }

    fn convert_messages(&self, messages: &[Message]) -> Vec<Value> {
        messages
            .iter()
            .map(|msg| {
                let mut m = json!({
                    "role": msg.role,
                });
                if let Some(ref content) = msg.content {
                    m["content"] = json!(content);
                } else {
                    m["content"] = json!(null);
                }
                if let Some(ref tool_calls) = msg.tool_calls {
                    let tc: Vec<Value> = tool_calls
                        .iter()
                        .map(|tc| {
                            json!({
                                "id": tc.id,
                                "type": "function",
                                "function": {
                                    "name": tc.name,
                                    "arguments": tc.arguments.to_string(),
                                }
                            })
                        })
                        .collect();
                    m["tool_calls"] = json!(tc);
                }
                if let Some(ref tool_call_id) = msg.tool_call_id {
                    m["tool_call_id"] = json!(tool_call_id);
                }
                m
            })
            .collect()
    }

    fn convert_tools(&self, tools: &[ToolDefinition]) -> Vec<Value> {
        tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    }
                })
            })
            .collect()
    }
}

#[async_trait]
impl Provider for OpenAiProvider {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<ProviderResponse, ProviderError> {
        let openai_messages = self.convert_messages(messages);
        let mut body = json!({
            "model": self.model,
            "messages": openai_messages,
            "max_tokens": self.max_tokens,
            "temperature": self.temperature,
        });
        if !tools.is_empty() {
            body["tools"] = json!(self.convert_tools(tools));
        }

        let url = format!("{}/chat/completions", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
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

        let content = resp["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string());

        let usage_input = resp["usage"]["prompt_tokens"]
            .as_u64()
            .unwrap_or(0) as u32;
        let usage_output = resp["usage"]["completion_tokens"]
            .as_u64()
            .unwrap_or(0) as u32;

        let mut tool_calls = Vec::new();
        let mut done = false;

        if let Some(tc_array) = resp["choices"][0]["message"]["tool_calls"].as_array() {
            for tc in tc_array {
                let id = tc["id"].as_str().unwrap_or_default().to_string();
                let name = tc["function"]["name"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let args_str = tc["function"]["arguments"].as_str().unwrap_or("{}");
                let arguments: Value =
                    serde_json::from_str(args_str).unwrap_or(json!({}));
                if name == "task_done" {
                    done = true;
                }
                tool_calls.push(super::ToolCall {
                    id,
                    name,
                    arguments,
                });
            }
        }

        Ok(ProviderResponse {
            content,
            tool_calls,
            usage: super::Usage {
                input_tokens: usage_input,
                output_tokens: usage_output,
            },
            reasoning: None,
            done,
        })
    }
}
