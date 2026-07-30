//! LLM 接入层。
//!
//! 两套协议：`openai`（/chat/completions）与 `anthropic`（/v1/messages）。
//! 两边 SSE 事件结构不同，各自解析后归一成 [`Delta`] 流。
//!
//! 所有 HTTP 调用都在 Rust 侧完成，API key 不进 webview。
//!
//! 加新协议：在此目录加 `xxx.rs` 实现 [`LlmProvider`]，在 [`Protocol`] 加枚举分支，
//! 在 [`dispatch`] 里接上即可。前端无需改动。

pub mod anthropic;
pub mod openai;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Sender;

use crate::config::{Protocol, Provider};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    /// "user" | "assistant"
    pub role: String,
    pub content: String,
}

#[derive(Clone, Debug)]
pub struct ChatRequest {
    pub model: String,
    pub system: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub max_tokens: u32,
    pub temperature: f32,
}

/// 归一后的流式增量。
///
/// 推理模型（DeepSeek 的 reasoning 系列、Anthropic 的 extended thinking）把思考过程
/// 放在与正文不同的字段里。两者必须分开：思考内容**不能**进释义正文，否则
/// `parsePartialJson` 拿到的是一大段自然语言；但也不能丢掉，不然长思考期间界面
/// 完全没动静，看起来就像卡死了。
#[derive(Debug, Clone)]
pub enum Delta {
    Text(String),
    /// 思考增量。只用于「思考中」的展示，不落库、不参与 JSON 解析。
    Reasoning(String),
}

pub trait LlmProvider {
    /// 流式对话。每个文本增量通过 `tx` 发出；正常结束时返回 Ok，调用方负责发终止事件。
    fn stream(
        &self,
        provider: &Provider,
        req: ChatRequest,
        tx: Sender<Delta>,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// 拉取可用模型列表。部分兼容端点不实现该接口，失败时调用方退回手填。
    fn list_models(
        &self,
        provider: &Provider,
    ) -> impl std::future::Future<Output = Result<Vec<String>>> + Send;
}

pub async fn stream(provider: &Provider, req: ChatRequest, tx: Sender<Delta>) -> Result<()> {
    match provider.protocol {
        Protocol::Openai => openai::Openai.stream(provider, req, tx).await,
        Protocol::Anthropic => anthropic::Anthropic.stream(provider, req, tx).await,
    }
}

pub async fn list_models(provider: &Provider) -> Result<Vec<String>> {
    match provider.protocol {
        Protocol::Openai => openai::Openai.list_models(provider).await,
        Protocol::Anthropic => anthropic::Anthropic.list_models(provider).await,
    }
}

/// 拼接端点，容忍 base_url 带或不带 `/v1` 后缀。
///
/// - `https://api.openai.com/v1` + `chat/completions` → `https://api.openai.com/v1/chat/completions`
/// - `https://api.openai.com`    + `chat/completions` → `https://api.openai.com/v1/chat/completions`
pub fn endpoint(base_url: &str, path: &str) -> String {
    let base = base_url.trim().trim_end_matches('/');
    if base.ends_with("/v1") {
        format!("{base}/{path}")
    } else {
        format!("{base}/v1/{path}")
    }
}

pub fn http_client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?)
}

/// 把非 2xx 响应转成带响应体的错误，方便用户在「测试连接」里看到端点真实报错。
pub async fn ensure_ok(resp: reqwest::Response) -> Result<reqwest::Response> {
    if resp.status().is_success() {
        return Ok(resp);
    }
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    let body = body.chars().take(500).collect::<String>();
    anyhow::bail!("HTTP {status}: {body}")
}
