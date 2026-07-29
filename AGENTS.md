# AGENTS.md

给在本仓库工作的 AI agent 与新加入的人看的架构说明。用户视角的安装与配置见 [README.md](./README.md)。

## 这是什么

macOS 上的 LLM 英语学习工具：跨 app 划词 → 光标旁弹窗给释义 → 同窗口多轮追问。

产品上的差异化只有一条：**结合上下文的释义**。词典给全部义项，本工具给"这句话里它是什么意思"。任何改动如果削弱这一点，方向就错了。

## 两阶段路线

| | 取词层 | 能拿到上下文 | 状态 |
| --- | --- | --- | --- |
| 阶段一 | PopClip → 本地 HTTP 端口 | 否 | 已实现 |
| 阶段二 | 自建 Accessibility API 取词 | 是 | 未开始 |

阶段一只为验证「划词 → 释义 → 追问」这套交互，取词层是借来的，随时可换。因此：

- 取词层与其余部分的唯一接口是 `POST /lookup`（`server.rs`），换实现不该动到 UI 与 LLM 层。
- `LookupPayload.context` 字段现在恒为 `None`，但**链路已经全程打通**（server → popup 事件 → `explain` 命令 → 提示词）。阶段二只需把它填上。
- 提示词里已经分了「有上下文」与「无上下文」两套措辞（`prompts.rs`）。

阶段二要做的：用 `kAXSelectedTextAttribute` 取选中文本，失败降级为模拟 `Cmd+C`；同时读 `kAXValue` + `kAXSelectedTextRange`，从整段文本切出前后文填进 `context`。参考 [`yetone/get-selected-text`](https://github.com/yetone/get-selected-text)。

## 目录

```
src-tauri/src/
  lib.rs          Tauri Builder、全部 #[tauri::command]、流式事件的编排
  config.rs       Settings 结构与磁盘读写（明文 JSON + 0600）
  server.rs       本地取词监听（axum，只绑 127.0.0.1）
  popup.rs        弹窗显示与光标定位
  state.rs        跨命令共享状态（暂存查询、当前流的句柄）
  prompts.rs      系统提示词
  llm/
    mod.rs        LlmProvider trait、协议分派、端点拼接
    openai.rs     OpenAI 兼容协议
    anthropic.rs  Anthropic 协议

src/
  main.tsx        按 URL 的 ?w= 参数分流到两个窗口
  global.css      主题变量与基础控件样式（含 dark mode）
  lib/
    types.ts      与 Rust 侧一一对应的类型
    api.ts        invoke 的薄封装，所有后端调用都走这里
    stream.ts     llm-stream 事件的单例监听与按 streamId 分发
    jsonish.ts    容错的增量 JSON 解析
  popup/          弹窗 UI
  settings/       设置窗口 UI
```

## 关键约定

**API key 不进 webview。** 所有 LLM HTTP 调用都在 Rust 侧完成，前端只通过 `invoke` 触发、通过事件收增量。`test_provider` / `list_models` 是例外——它们由设置页传入完整 `Provider`（含 key），因为此时 key 本来就是用户正在这个表单里输入的。

**明文存 key 是用户明确选择的方案**，不是疏漏。文件权限收紧到 0600，README 里有安全说明。不要擅自改回 Keychain。

**结构化输出不用 `response_format` / tool use。** 多数 OpenAI 兼容端点（Ollama、LM Studio、各类中转）不支持或行为不一致。释义靠提示词约束 JSON，前端 `jsonish.ts` 做容错增量解析。这是跨协议唯一稳的做法，改用原生结构化输出会让一半端点挂掉。

**用本地端口而不是 `enassistant://` deep link。** macOS 不支持运行时注册 URL scheme，deep link 只有装到 `/Applications` 的打包 .app 才能测，`tauri dev` 下无法调试。另外 PopClip 的 url action 会顺带打开浏览器标签页，所以片段用的是 shell script action。

**两个窗口常驻不销毁。** 关闭按钮只 `hide()`（`lib.rs` 的 `on_window_event`），避免每次查询重建 webview。

## 加一个新的 LLM 协议

1. `src-tauri/src/llm/` 下新建 `xxx.rs`，实现 `LlmProvider` 的 `stream` 与 `list_models`
2. `config.rs` 的 `Protocol` 枚举加分支
3. `llm/mod.rs` 的 `stream` / `list_models` 分派函数加分支
4. `src/lib/types.ts` 的 `Protocol`、`PROTOCOL_LABELS`、`PROTOCOL_DEFAULT_BASE_URL` 同步

前端其余部分不需要改。

`endpoint()` 会容忍 base_url 结尾带不带 `/v1`，新协议实现里直接用它拼路径。

## 流式的数据流

```
前端 startStream(streamId) ──invoke──> explain / chat_turn
                                          │ spawn 后台任务
                                          ▼
                              llm::stream ──mpsc──> forwarder
                                                        │ emit_to("popup", "llm-stream")
                                                        ▼
                        stream.ts 按 streamId 分发 ──> onDelta / onDone / onError
```

- 命令**立即返回**，不等流跑完；成功与否都通过事件告知。
- 发起新流会 `abort` 上一条（`state::replace_stream`），避免两股增量交错渲染。
- `llm::stream` 返回时 `tx` 被丢弃，forwarder 随之自然结束——不要给它加显式的终止信号。

## 开发

```bash
npm run tauri dev             # 前端热更新；Rust 改动触发重编重启
npm run build                 # 前端类型检查 + 构建
npm test                      # vitest
cd src-tauri && cargo test
```

现有测试覆盖两处纯逻辑，都是真机上不好复现的：

- `popup.rs` 的 `place()` — 弹窗越界钳制。真机没法把鼠标挪到屏幕角落来测。
  **注意 `cursor_position()` 返回物理坐标而 `monitor_from_point()` 收逻辑坐标**，混用会让光标偏下时找不到显示器，钳制被静默跳过，弹窗掉出屏幕。
- `jsonish.ts` 的 `parsePartialJson()` — 半截 JSON 的容错解析。

不接真 key 也能验证两条协议：起一个 mock SSE 端点，把 provider 的 `baseUrl` 指过去即可。
`openai` 分支要 `data: {"choices":[{"delta":{"content":...}}]}` + `data: [DONE]`；
`anthropic` 分支要带 `event:` 名的 `content_block_delta` + `message_stop`。

手工触发一次查询（不装 PopClip 也能测）：

```bash
curl -X POST http://127.0.0.1:8765/lookup --data-urlencode "q=take on"
# 阶段二的上下文参数也已经能收：
curl -X POST http://127.0.0.1:8765/lookup \
  --data-urlencode "q=take on" \
  --data-urlencode "context=She had to take on more responsibility."
```

## 尚未实现

生词本、复习（FSRS）、本地词典即时层、结果缓存。都在阶段二之后，见 README 的路线图。
