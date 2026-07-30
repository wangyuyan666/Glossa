//! OpenAI 兼容协议。
//!
//! 覆盖 OpenAI 官方、DeepSeek、硅基流动、OpenRouter、Groq、Ollama、LM Studio 等。
//! SSE 形如 `data: {"choices":[{"delta":{"content":"..."}}]}`，终止符 `data: [DONE]`。

use anyhow::Result;
use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio::sync::mpsc::Sender;

use super::{endpoint, ensure_ok, http_client, ChatRequest, Delta, LlmProvider};
use crate::config::Provider;

pub struct Openai;

impl LlmProvider for Openai {
    async fn stream(&self, provider: &Provider, req: ChatRequest, tx: Sender<Delta>) -> Result<()> {
        let mut messages: Vec<Value> = Vec::with_capacity(req.messages.len() + 1);
        // openai 协议把 system 当作首条消息，而非顶层字段。
        if let Some(system) = &req.system {
            messages.push(json!({ "role": "system", "content": system }));
        }
        for m in &req.messages {
            messages.push(json!({ "role": m.role, "content": m.content }));
        }

        let body = json!({
            "model": req.model,
            "messages": messages,
            "max_tokens": req.max_tokens,
            "temperature": req.temperature,
            "stream": true,
        });

        let resp = http_client()?
            .post(endpoint(&provider.base_url, "chat/completions"))
            .bearer_auth(&provider.api_key)
            .json(&body)
            .send()
            .await?;
        let resp = ensure_ok(resp).await?;

        let mut events = resp.bytes_stream().eventsource();
        let mut got_text = false;
        let mut finish_reason: Option<String> = None;

        while let Some(event) = events.next().await {
            let event = event?;
            if event.data.trim() == "[DONE]" {
                break;
            }
            let Ok(value) = serde_json::from_str::<Value>(&event.data) else {
                continue;
            };
            // 部分兼容端点会在流里塞 error 对象而不是返回非 2xx。
            if let Some(err) = value.get("error") {
                anyhow::bail!("{err}");
            }

            let choice = &value["choices"][0];
            if let Some(reason) = choice["finish_reason"].as_str() {
                finish_reason = Some(reason.to_string());
            }

            // 推理模型的思考走 `reasoning_content`，此时 `content` 是 null。
            // 只认 content 的话，思考期间一个增量都收不到。
            let reasoning = choice["delta"]["reasoning_content"]
                .as_str()
                .unwrap_or_default();
            if !reasoning.is_empty()
                && tx
                    .send(Delta::Reasoning(reasoning.to_string()))
                    .await
                    .is_err()
            {
                // 接收端已关闭（窗口关了或发起了新查询），停止拉流。
                break;
            }

            let text = choice["delta"]["content"].as_str().unwrap_or_default();
            if !text.is_empty() {
                got_text = true;
                if tx.send(Delta::Text(text.to_string())).await.is_err() {
                    break;
                }
            }
        }

        // 推理模型的思考也算 completion token：额度不够时思考写满就截断，正文一个字都没有。
        // 这里不报错的话，上层只会看到一次「成功但空白」的查询，最难排查的那种失败。
        if !got_text && finish_reason.as_deref() == Some("length") {
            anyhow::bail!(
                "模型把 {} token 的额度全用在思考上，没输出正文。换个非推理模型，或调高上限",
                req.max_tokens
            );
        }
        Ok(())
    }

    async fn list_models(&self, provider: &Provider) -> Result<Vec<String>> {
        let resp = http_client()?
            .get(endpoint(&provider.base_url, "models"))
            .bearer_auth(&provider.api_key)
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
