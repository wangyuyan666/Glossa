//! Anthropic 协议。
//!
//! 覆盖 Anthropic 官方及其兼容代理。
//! SSE 事件带 `event:` 名，文本增量在 `content_block_delta` 的 `delta.text`，终止事件 `message_stop`。
//! 鉴权走 `x-api-key` 头而非 Bearer，且必须带 `anthropic-version`。

use anyhow::Result;
use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio::sync::mpsc::Sender;

use super::{endpoint, ensure_ok, http_client, ChatRequest, Delta, LlmProvider};
use crate::config::Provider;

const API_VERSION: &str = "2023-06-01";

pub struct Anthropic;

impl LlmProvider for Anthropic {
    async fn stream(&self, provider: &Provider, req: ChatRequest, tx: Sender<Delta>) -> Result<()> {
        let messages: Vec<Value> = req
            .messages
            .iter()
            .map(|m| json!({ "role": m.role, "content": m.content }))
            .collect();

        let mut body = json!({
            "model": req.model,
            "messages": messages,
            "max_tokens": req.max_tokens,
            "temperature": req.temperature,
            "stream": true,
        });
        // anthropic 协议把 system 放顶层字段，不进 messages 数组。
        if let Some(system) = &req.system {
            body["system"] = json!(system);
        }

        let resp = http_client()?
            .post(endpoint(&provider.base_url, "messages"))
            .header("x-api-key", &provider.api_key)
            .header("anthropic-version", API_VERSION)
            .json(&body)
            .send()
            .await?;
        let resp = ensure_ok(resp).await?;

        let mut events = resp.bytes_stream().eventsource();
        while let Some(event) = events.next().await {
            let event = event?;
            match event.event.as_str() {
                "content_block_delta" => {
                    let Ok(value) = serde_json::from_str::<Value>(&event.data) else {
                        continue;
                    };
                    let delta = &value["delta"];
                    // extended thinking 开着时思考走 `thinking_delta`，正文走 `text_delta`。
                    // 两者归一到不同的 Delta 变体：思考不能混进释义 JSON。
                    let thinking = delta["thinking"].as_str().unwrap_or_default();
                    if !thinking.is_empty()
                        && tx
                            .send(Delta::Reasoning(thinking.to_string()))
                            .await
                            .is_err()
                    {
                        break;
                    }

                    let text = delta["text"].as_str().unwrap_or_default();
                    if !text.is_empty() && tx.send(Delta::Text(text.to_string())).await.is_err() {
                        break;
                    }
                }
                "error" => {
                    anyhow::bail!("{}", event.data);
                }
                "message_stop" => break,
                _ => {}
            }
        }
        Ok(())
    }

    async fn list_models(&self, provider: &Provider) -> Result<Vec<String>> {
        let resp = http_client()?
            .get(endpoint(&provider.base_url, "models"))
            .header("x-api-key", &provider.api_key)
            .header("anthropic-version", API_VERSION)
            .send()
            .await?;
        let value: Value = ensure_ok(resp).await?.json().await?;

        let mut models: Vec<String> = value["data"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m["id"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        models.sort();
        Ok(models)
    }
}
