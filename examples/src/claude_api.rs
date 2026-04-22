// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Minimal Claude API client with streaming and tool use.

use std::io::BufRead;
use std::sync::mpsc;

use serde::{Deserialize, Serialize};

/// A message in the conversation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: MessageContent,
}

/// Message content — either a string or structured content blocks.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

/// One content block in a message.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

/// A tool definition for the API.
#[derive(Clone, Debug, Serialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Events streamed back from the API.
#[derive(Debug)]
pub enum StreamEvent {
    /// A chunk of text from the assistant.
    TextDelta(String),
    /// The assistant wants to call a tool.
    ToolCall {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// The response is complete.
    Done,
    /// An error occurred.
    Error(String),
}

/// Request body for the Messages API.
#[derive(Serialize)]
struct ApiRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: &'a [Message],
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: &'a Vec<Tool>,
    stream: bool,
}

/// Sends a streaming request to the Claude API in a background thread.
///
/// Returns a receiver that yields `StreamEvent`s as they arrive.
/// The API key is read from the `ANTHROPIC_API_KEY` environment variable.
pub fn send_streaming(
    messages: Vec<Message>,
    tools: Vec<Tool>,
    system: Option<String>,
) -> mpsc::Receiver<StreamEvent> {
    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        let api_key = match std::env::var("ANTHROPIC_API_KEY") {
            Ok(key) => key,
            Err(_) => {
                let _ = tx.send(StreamEvent::Error(
                    "ANTHROPIC_API_KEY not set".into(),
                ));
                return;
            }
        };

        let body = ApiRequest {
            model: "claude-sonnet-4-20250514",
            max_tokens: 1024,
            messages: &messages,
            system: system.as_deref(),
            tools: &tools,
            stream: true,
        };

        let body_json = match serde_json::to_vec(&body) {
            Ok(j) => j,
            Err(e) => {
                let _ = tx.send(StreamEvent::Error(format!("serialize: {e}")));
                return;
            }
        };

        let response = ureq::post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .send(&body_json);

        let response = match response {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.send(StreamEvent::Error(format!("request: {e}")));
                return;
            }
        };

        let reader = std::io::BufReader::new(response.into_body().into_reader());
        let mut current_tool_id = String::new();
        let mut current_tool_name = String::new();
        let mut current_tool_input = String::new();
        let mut in_tool_use = false;

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    let _ = tx.send(StreamEvent::Error(format!("read: {e}")));
                    break;
                }
            };

            if !line.starts_with("data: ") {
                continue;
            }
            let data = &line[6..];
            if data == "[DONE]" {
                break;
            }

            let event: serde_json::Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let event_type = event["type"].as_str().unwrap_or("");
            match event_type {
                "content_block_start" => {
                    if let Some(block) = event.get("content_block")
                        && block["type"].as_str() == Some("tool_use")
                    {
                        in_tool_use = true;
                        current_tool_id =
                            block["id"].as_str().unwrap_or("").to_string();
                        current_tool_name =
                            block["name"].as_str().unwrap_or("").to_string();
                        current_tool_input.clear();
                    }
                }
                "content_block_delta" => {
                    if let Some(delta) = event.get("delta") {
                        if delta["type"].as_str() == Some("text_delta")
                            && let Some(text) = delta["text"].as_str()
                        {
                            let _ = tx.send(StreamEvent::TextDelta(text.to_string()));
                        } else if delta["type"].as_str() == Some("input_json_delta")
                            && let Some(partial) = delta["partial_json"].as_str()
                        {
                            current_tool_input.push_str(partial);
                        }
                    }
                }
                "content_block_stop" if in_tool_use => {
                    let input: serde_json::Value =
                        serde_json::from_str(&current_tool_input)
                            .unwrap_or(serde_json::Value::Null);
                    let _ = tx.send(StreamEvent::ToolCall {
                        id: current_tool_id.clone(),
                        name: current_tool_name.clone(),
                        input,
                    });
                    in_tool_use = false;
                }
                "message_stop" => {
                    let _ = tx.send(StreamEvent::Done);
                    break;
                }
                "error" => {
                    let msg = event["error"]["message"]
                        .as_str()
                        .unwrap_or("unknown error");
                    let _ = tx.send(StreamEvent::Error(msg.to_string()));
                    break;
                }
                _ => {}
            }
        }

        let _ = tx.send(StreamEvent::Done);
    });

    rx
}
