use crate::config::SummarizationConfig;
use crate::provider::{Message, Provider};
use crate::trace::TraceLogger;
use std::sync::Arc;

pub struct Summarizer {
    config: SummarizationConfig,
    provider: Arc<dyn Provider>,
    trace: Arc<TraceLogger>,
    runs: u32,
    tokens_saved: u32,
}

impl Summarizer {
    pub fn new(
        config: SummarizationConfig,
        provider: Arc<dyn Provider>,
        trace: Arc<TraceLogger>,
    ) -> Self {
        Self {
            config,
            provider,
            trace,
            runs: 0,
            tokens_saved: 0,
        }
    }

    pub fn should_summarize(&self, messages: &[Message]) -> bool {
        if !self.config.enabled {
            return false;
        }
        let estimated_tokens: u32 = messages
            .iter()
            .map(|m| m.content.as_ref().map(|c| c.len() as u32).unwrap_or(0) / 4)
            .sum();
        estimated_tokens > self.config.trigger_tokens
    }

    pub async fn summarize(
        &mut self,
        messages: &mut Vec<Message>,
        iteration: u32,
    ) -> Result<(), String> {
        if messages.is_empty() {
            return Ok(());
        }

        let total_chars: usize = messages
            .iter()
            .map(|m| m.content.as_ref().map(|c| c.len()).unwrap_or(0))
            .sum();
        let tokens_before = (total_chars / 4) as u32;

        let system_msg = messages.first().filter(|m| m.role == "system").cloned();

        let keep_count = (self.config.keep_tokens as usize * 4) / 100;
        let keep_count = keep_count.max(2).min(messages.len());

        let recent_start = messages.len().saturating_sub(keep_count);

        let older: Vec<Message> = if system_msg.is_some() && recent_start > 1 {
            messages[1..recent_start].to_vec()
        } else if recent_start > 0 {
            messages[..recent_start].to_vec()
        } else {
            return Ok(());
        };

        if older.is_empty() {
            return Ok(());
        }

        let older_text: String = older
            .iter()
            .filter_map(|m| {
                m.content
                    .as_ref()
                    .map(|c| format!("[{}]: {}", m.role, c))
            })
            .collect::<Vec<_>>()
            .join("\n");

        let summarization_prompt = format!(
            "Summarize the following conversation history concisely, preserving key decisions, \
             facts, and context needed to continue the task. Keep the summary under {} tokens.\n\n{}",
            self.config.trim_tokens, older_text
        );

        let model = self
            .config
            .model
            .as_deref()
            .unwrap_or("summarization model");

        let _ = model;

        let sum_messages = vec![
            Message::system(
                "You are a summarization assistant. Produce concise, accurate summaries.".to_string(),
            ),
            Message::user(summarization_prompt),
        ];

        let response = self
            .provider
            .complete(&sum_messages, &[])
            .await
            .map_err(|e| format!("Summarization LLM call failed: {}", e))?;

        let summary_text = response
            .content
            .unwrap_or_else(|| "Previous context was summarized but produced no output.".to_string());

        let recent: Vec<Message> = messages[recent_start..].to_vec();

        let mut new_messages = Vec::new();
        if let Some(sys) = system_msg {
            new_messages.push(sys);
        }
        new_messages.push(Message::system(format!(
            "[Conversation summary]: {}",
            summary_text
        )));
        new_messages.extend(recent);

        let new_chars: usize = new_messages
            .iter()
            .map(|m| m.content.as_ref().map(|c| c.len()).unwrap_or(0))
            .sum();
        let tokens_after = (new_chars / 4) as u32;

        self.trace.log_summarization(iteration, tokens_before, tokens_after);

        let saved = tokens_before.saturating_sub(tokens_after);
        self.runs += 1;
        self.tokens_saved += saved;

        *messages = new_messages;
        Ok(())
    }

    pub fn runs(&self) -> u32 {
        self.runs
    }

    pub fn tokens_saved(&self) -> u32 {
        self.tokens_saved
    }
}
