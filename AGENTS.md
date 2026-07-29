# AGENTS.md

给在本仓库工作的 AI agent 与新加入的人看的架构说明。用户视角的安装与配置见 [README.md](./README.md)。

## 这是什么

macOS 上的 LLM 英语学习工具：跨 app 划词 → 主窗口给释义 → 多轮追问 → 落进历史。

产品上的差异化只有一条：**结合上下文的释义**。词典给全部义项，本工具给"这句话里它是什么意思"。任何改动如果削弱这一点，方向就错了。

## 两阶段路线

| | 取词层 | 能拿到上下文 | 状态 |
| --- | --- | --- | --- |
| 阶段一 | PopClip → 本地 HTTP 端口 | 否 | 已实现 |
| 阶段一补 | PopClip 扩展一键安装 | 否 | 已实现 |
| 阶段二 | 全局快捷键 + Accessibility API 取词 | 是 | **TODO** |

阶段一只为验证「划词 → 释义 → 追问」这套交互，取词层是借来的，随时可换。因此：

- 取词层与其余部分的唯一接口是 `POST /lookup`（`server.rs`），换实现不该动到 UI 与 LLM 层。
- `LookupPayload.context` 字段现在恒为 `None`，但**链路已经全程打通**（server → lookup 事件 → `explain` 命令 → 提示词）。阶段二只需把它填上。
- 提示词里已经分了「有上下文」与「无上下文」两套措辞（`prompts.rs`）。

## 阶段二 TODO：快捷键取词

两条取词路径**并存**，不互斥——PopClip 要点条上的图标，快捷键要按键，同一次选词不可能都触发。快捷键路径能拿到上下文，PopClip 路径作为无权限时的降级。

计划中的模块：

```
src-tauri/src/capture/
  mod.rs         AX 优先，Cmd+C 兜底，统一返回 Selection{ text, context }
  ax.rs          kAXSelectedTextAttribute / kAXValue / kAXSelectedTextRange
  clipboard.rs   Cmd+C 兜底：存旧剪贴板 → CGEventPost → 读 → 还原
  permission.rs  AXIsProcessTrustedWithOptions + 跳系统设置面板
  context.rs     从整段文本切前后文（纯函数，可单测）
```

默认快捷键 `⌘⇧E`，设置里可改。开关默认关闭，**打开时才申请辅助功能权限**——不打开就完全不碰权限。

已经踩明白、别再重新推一遍的坑：

- **AX 的 range 是 UTF-16 code unit 索引**，Rust `String` 是 UTF-8。直接拿 range 当字节偏移切，中英混排必然切在字符中间 panic。要先转 `Vec<u16>` 再切回来。
- **取词必须在唤起主窗口之前**。先显示窗口会让 EnAssistant 变成前台 app，接着读 AX 就读到我们自己的窗口了。
- **前台 app 是 EnAssistant 自己时要跳过**（用户在主窗口里手滑按了快捷键）。
- **`RegisterEventHotKey` 优先级低**，被系统或其他 app 占用时注册会失败。必须在设置页明确报错，静默失败会让用户以为取词坏了。
- **TCC 权限按二进制路径 + 签名记账**。`tauri dev` 跑的是 `target/debug/enassistant` 裸二进制而非 `.app` bundle，每次改 Rust 重编都换了二进制，授权可能失效或反复要权限。AX 相关功能要在 `npm run tauri build` 的产物上验证。这和 deep link 那个坑是同一类问题。
- 权限只需要**辅助功能**一个。CGEventTap 监听选区还要额外的**输入监控**权限，两次授权流程会明显拉高流失——这也是选快捷键而非选区监听的原因之一。

上下文能力是**尽力而为**：Electron / Chrome 系 app 的 AX 实现参差，`kAXValue` 经常拿不到整段文本；Cmd+C 兜底路径永远拿不到上下文（剪贴板里只有选中内容）。这些情况降级成 `context: None` 即可，提示词本来就分了两套。

参考实现：[`yetone/get-selected-text`](https://github.com/yetone/get-selected-text)（Rust，只取词不取上下文）。

## 目录

```
src-tauri/src/
  lib.rs          Tauri Builder、全部 #[tauri::command]、流式事件的编排
  history.rs      查询历史（SQLite）
  popclip.rs      PopClip 扩展的生成与安装（package 一键 / snippet 手动）
  config.rs       Settings 结构与磁盘读写（明文 JSON + 0600）
  server.rs       本地取词监听（axum，只绑 127.0.0.1）
  windows.rs      窗口显示、label 常量、LookupPayload
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
  lookup/         查词逻辑与 UI
    useLookup.ts  一次查询的完整状态机：流式释义 + 多轮追问 + 落库
    LookupView    释义卡片 + 对话的滚动区
    AskBox        追问输入框
  main/           主窗口：左历史栏 + 右查词区
  settings/       设置窗口 UI（CaptureSection 是取词那一节）
```

## 单词模式与句子模式

选中一整句时，输出必须是**整句翻译 + 难点**，不能退化成挑一个词解释。

判定在 Rust 侧（`prompts.rs` 的 `is_sentence`），不交给模型——确定性、可单测、不多花一次调用。规则：六个词以上，或三词以上且带句末标点。

两套提示词、两套 JSON 结构：

| | 单词 / 短语 | 句子 |
| --- | --- | --- |
| 字段 | `word` `phonetic` `pos` `senseHere` `why` `collocations[]` `example` | `translation` `structure` `keyPoints[{term,note}]` |
| 上下文 | 用 `context` 定位此处义项 | 不用——句子本身就是自己的上下文 |

**别把两套字段合成一套。** 句子走单词 schema 时，`word`（词条原形）/`phonetic`/`pos` 会逼着模型从句子里挑一个词来填，症状就是「选了一整句却只翻译其中一个单词」。这是本项目实际踩过的 bug。

前端 `ExplanationCard` 按字段存在性分流，不需要额外的模式标记——`parsePartialJson` 返回的本来就是部分对象，流式渲染天然继续工作。

`history.rs` 的 `extract_sense` 取侧栏副标题时 `senseHere` → `translation` 依次回退，否则句子记录在侧栏是空的一行。

## 两个窗口

| label | 用途 |
| --- | --- |
| `main` | 主窗口：左历史栏 + 右查词区。启动即开，划词也唤它 |
| `settings` | 设置 |

曾经有第三个 `popup` 窗口（无边框、定位到光标旁）。删掉了：划词直接唤主窗口，界面收敛成两个。**别再加回来**——如果要做轻量浮层，先想清楚它和主窗口的分工，以及跨 Space 时 `set_focus` 会切换桌面这件事。

窗口都是启动时建好、平时隐藏，关闭按钮只 `hide()` 不销毁（`lib.rs` 的 `on_window_event`），避免下次用到要重建 webview。因此 `RunEvent::Reopen` 必须处理——不然主窗口关掉后点 Dock 图标就再也打不开了。

划词的载荷经 `windows::present()` 送到主窗口。冷启动时事件可能早于 React 挂载，所以 `state.rs` 里留了一份暂存，`Main` 挂载时主动 `takePendingLookup()` 兜一次。

流式事件用 `app.emit()` **广播**，前端按 `streamId` 分发。

## 历史存储

`history.rs`，SQLite 在 `~/Library/Application Support/EnAssistant/history.db`。

选 SQLite 不选 JSON 是为了路线图：生词本 + FSRS 复习要按到期时间查、按熟练度排序，那必须有真正的存储层。

```sql
lookups(id, text, context, explanation, created_at)
turns(id, lookup_id → lookups.id ON DELETE CASCADE, seq, role, content)
```

- `explanation` 存**释义的原始 JSON 字符串**，不拆字段。前端恢复历史时仍走 `parsePartialJson`，和流式路径共用同一套渲染——模型输出被截断的旧记录也能正常显示。
- 级联删除依赖 `PRAGMA foreign_keys = ON`，这句在 `migrate()` 里，别删。
- 这是**历史流水**不是词表：同一个词查两次记两条。去重是生词本的事。

**落库时机**：

| | 谁写 | 何时 |
| --- | --- | --- |
| 释义 | Rust（`spawn_stream`） | 流成功结束时。全文本来就在 Rust 手里，不用让前端传回来 |
| 追问的提问 | 前端 | 发起请求**前**。它和流的成败无关，流挂了问题也该留着 |
| 追问的回答 | 前端 | 流成功结束后 |

`lookupId` 由前端生成并传给 `explain`，追问用同一个 id 往 `turns` 追加，所以一条历史点开就是完整会话。

落库失败只记日志，不影响用户看到释义。

## PopClip 的两种安装形式

别搞混，两者的要求正相反：

| | snippet | package |
| --- | --- | --- |
| 形态 | 一段 YAML 文本 | `.popclipext` 目录 + `Config.yaml` |
| `#popclip` 头行 | **必须有**，是识别标志 | **不要有** |
| 安装方式 | 用户**选中**整段，PopClip 条上出现 Install Extension | `open` 该目录，PopClip 弹确认框 |
| 我们用在 | 手动安装（兜底） | 一键安装 |

两种形式都带 `identifier: com.peter.enassistant`，PopClip 据此认出是同一个扩展——改端口后重装会覆盖，而不是多出一个重复图标。

一键安装依赖 macOS 文件关联，Setapp 版、多版本共存、关联被别的软件抢走都可能失效，所以**手动路径必须保留**。`open` 之前先探测 PopClip 是否存在，否则 macOS 会弹「没有可打开此文件的应用」这种让人摸不着头脑的错误。

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
                                                        │ emit("llm-stream")
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

- `history.rs` 的 `migrate()` — 含老库迁移（曾有过一个 `source` 列，记查询从弹窗还是主窗口发起；弹窗删掉后该列失去意义且是 NOT NULL，会让新的 INSERT 失败，所以必须真删）。
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
