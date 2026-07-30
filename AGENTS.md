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
- **取词必须在唤起主窗口之前**。先显示窗口会让 Glossa 变成前台 app，接着读 AX 就读到我们自己的窗口了。
- **前台 app 是 Glossa 自己时要跳过**（用户在主窗口里手滑按了快捷键）。
- **`RegisterEventHotKey` 优先级低**，被系统或其他 app 占用时注册会失败。必须在设置页明确报错，静默失败会让用户以为取词坏了。
- **TCC 权限按二进制路径 + 签名记账**。`tauri dev` 跑的是 `target/debug/Glossa` 裸二进制而非 `.app` bundle，每次改 Rust 重编都换了二进制，授权可能失效或反复要权限。AX 相关功能要在 `npm run tauri build` 的产物上验证。这和 deep link 那个坑是同一类问题。
- 权限只需要**辅助功能**一个。CGEventTap 监听选区还要额外的**输入监控**权限，两次授权流程会明显拉高流失——这也是选快捷键而非选区监听的原因之一。

上下文能力是**尽力而为**：Electron / Chrome 系 app 的 AX 实现参差，`kAXValue` 经常拿不到整段文本；Cmd+C 兜底路径永远拿不到上下文（剪贴板里只有选中内容）。这些情况降级成 `context: None` 即可，提示词本来就分了两套。

参考实现：[`yetone/get-selected-text`](https://github.com/yetone/get-selected-text)（Rust，只取词不取上下文）。

## 目录

```
src-tauri/src/
  lib.rs          Tauri Builder、全部 #[tauri::command]、流式事件的编排
  history.rs      查询历史（SQLite）
  templates.rs    提示词模板：内置正文、变量渲染、静态检查
  prompts.rs      判定单词/句子、拼首轮用户消息
  popclip.rs      PopClip 扩展的生成与安装（package 一键 / snippet 手动）
  config.rs       Settings 结构与磁盘读写（明文 JSON + 0600）
  server.rs       本地取词监听（axum，只绑 127.0.0.1）
  windows.rs      窗口显示、label 常量、LookupPayload
  first_mouse.rs  绕过 WKWebView 吞掉首次点击的上游 bug（objc runtime，macOS only）
  state.rs        跨命令共享状态（暂存查询、当前流的句柄）
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
    imeClick.ts   补回输入法组词时被吞掉的第一次点击（macOS）
  lookup/         查词逻辑与 UI
    useLookup.ts  一次查询的完整状态机：流式释义 + 多轮追问 + 落库
    LookupView    释义卡片 + 对话的滚动区
    AskBox        追问输入框
  main/           主窗口：左历史栏 + 右查词区
  settings/       设置窗口 UI
    CaptureSection  取词那一节
    PromptSection   提示词模板的选用与管理
    TemplateEditor  单个模板的编辑 + 静态检查 + 实测
  ui/
    icons.tsx     内联 SVG 图标，两个窗口共用
```

两个窗口共用一份构建产物，CSS 也是同一份：`settings.css` 里的类名带 `settings-`
前缀就是为此，不带前缀的通用类会和主窗口串。

## 提示词模板

`templates.rs`。四类（`word` / `sentence` / `translate` / `chat`）各自独立选用——用户想只改释义风格、保留默认对话，不该被迫连对话一起写。

**内置模板留在代码里，不写进 `settings.json`**，配置只记「当前选了哪个 id」。内置提示词以后还会改；把副本存进用户配置的话，升级后用户还在跑旧提示词，而且完全看不出来。`config::sanitize()` 在落盘前剔除内置副本、并把 `builtin` 标记强制置 false。

选中的模板被删掉、或指向了错误的 kind（比如把对话模板选进释义位置），都回落到内置——提示词是主流程的一环，任何情况下不能没有。

变量只有两个：`{{nativeLanguage}}`、`{{context}}`。**选中的内容不是变量**，它在 user message 里发，模板只负责「怎么解释」。未知变量渲染时原样保留而不是替换成空串——静默吞掉的话，用户拼错了只会觉得「模板好像没生效」。

**检测分两层，缺一不可**：

| | 能抓什么 | 抓不到什么 |
| --- | --- | --- |
| 静态检查（`templates::check`） | 拼错变量、漏写必需字段名、正文过长 | 模型是否真的照做 |
| 实测（`lib.rs` 的 `probe_template`） | **模型不听话**——自定义提示词最常见的失败方式 | — |

实测用固定样例（`prompts::probe_input`）真发一次请求，核对返回 JSON 里必需字段是否齐全，并把原始输出展示给用户。这不是锦上添花，是用户唯一的自查手段。

自定义提示词的固有代价：卡片渲染稳定是因为内置提示词经过调试，用户写的模板漏字段、或被模型忽略，卡片就会缺块。这不是能修的 bug。

## 三种释义模式

选中一整句时，输出必须是**整句翻译 + 难点**，不能退化成挑一个词解释。选中的内容不是英文时，那是「这句话英语怎么说」，不是「这是什么意思」。

判定全在 Rust 侧（`prompts.rs` 的 `kind_for`），不交给模型——确定性、可单测、不多花一次调用。**顺序不能反**：

1. `looks_foreign` — 出现任意一个非拉丁字母（码位 > U+024F 的字母字符）就走翻译。中文没有空格，`is_sentence` 数出来永远是 1 个词，不先拦这一道的话整段中文会落进单词模板。`café` / `naïve` 是拉丁字母，不会误判。
2. `is_sentence` — 六个词以上，或三词以上且带句末标点。

三套提示词、三套 JSON 结构：

| | 单词 / 短语 | 句子 | 译成英文 |
| --- | --- | --- | --- |
| 字段 | `grammar{issue,corrected}` `word` `phonetic` `pos` `senseHere` `why` `collocations[]` `example` | `grammar{issue,corrected}` `translation` `structure` `keyPoints[{term,note}]` | `english` `wordChoice[{term,note}]` `alternatives[{text,when}]` |
| 上下文 | 用 `context` 定位此处义项 | 不用——句子本身就是自己的上下文 | 不用 |

**别把几套字段合成一套。** 句子或中文走单词 schema 时，`word`（词条原形）/`phonetic`/`pos` 会逼着模型挑个词来填、给中文标音标，症状就是「选了一整句却只翻译其中一个单词」。这是本项目实际踩过的 bug。

翻译模式的重点是 `wordChoice`（**为什么用这些词**），不是译文本身——译文别的工具也给得出，选词理由才是这里要教的。`alternatives` 给语域/语气不同的备选。翻译模式**没有 `grammar`**：原文本来就不是英文。

`grammar` 是选中内容的拼写 / 语法纠错，排在两套释义 schema 的最前，没毛病时两个子字段都是空串、卡片不渲染这一块。

- **两套都有**，因为 `is_sentence` 挡不住短错句：`how's is goging today` 才 4 个词又没句末标点，判成词组走单词模板，只加在句子模板上等于没加。也正因为两套都有，**它不能用来判断是词还是句**，`ExplanationCard` 分流只看各自独有的字段。
- **不在** `required_fields` 里：加进去会让用户已有的自建模板立刻变 error，实测探针也会在「样例本来就没语法错」时误判模型漏了字段。

前端 `ExplanationCard` 按字段存在性分流（`english`/`wordChoice`/`alternatives` → 翻译，`translation`/`structure`/`keyPoints` → 句子，其余 → 单词），不需要额外的模式标记——`parsePartialJson` 返回的本来就是部分对象，流式渲染天然继续工作。

`history.rs` 的 `extract_sense` 取侧栏副标题时 `senseHere` → `translation` → `english` 依次回退，少一个回退那类记录在侧栏就是空的一行。

## 两个窗口

| label | 用途 |
| --- | --- |
| `main` | 主窗口：左历史栏 + 右查词区。启动即开，划词也唤它 |
| `settings` | 设置 |

曾经有第三个 `popup` 窗口（无边框、定位到光标旁）。删掉了：划词直接唤主窗口，界面收敛成两个。**别再加回来**——如果要做轻量浮层，先想清楚它和主窗口的分工，以及跨 Space 时 `set_focus` 会切换桌面这件事。

窗口都是启动时建好、平时隐藏，关闭按钮只 `hide()` 不销毁（`lib.rs` 的 `on_window_event`），避免下次用到要重建 webview。因此 `RunEvent::Reopen` 必须处理——不然主窗口关掉后点 Dock 图标就再也打不开了。

应用必须保持**单实例**，`tauri-plugin-single-instance` 要放在 plugin 列表最前面。这里不只是一般的桌面应用习惯：本 app 占固定 127.0.0.1 端口。没有单实例时，第二个 `tauri dev` 进程会出现「端口绑定失败但窗口照开」的半残状态，两套一模一样的窗口并存，点到哪套和终端日志完全对不上。第二次启动现在只唤回已有主窗口。

### 首次点击

NSWindow 默认 `acceptsFirstMouse: NO`：app 不在前台时，落到窗口上的第一次点击被系统吃掉、只用来激活 app，**不派发给 WKWebView**，React 的 onClick 根本不触发。症状是任何控件第一下都没反应、点第二下才行，最容易被误认成设置按钮的 bug——实际上和按钮无关，而且很难往窗口配置上想。

**`tauri.conf.json` 里的 `acceptFirstMouse: true` 治不了这个。** 别以为配上就完事了（这个仓库犯过一次，还把结论写进了文档）：

- tao 给窗口 content view 覆写了 `acceptsFirstMouse:` 恒返回 YES；
- wry 给 `WryWebView`（WKWebView 的子类）也覆写了，按配置返回其内部 ivar；
- 实测 AppKit 问的就是 `WryWebView`，但单靠配置时它仍吞第一次点击。这是上游已有的「按窗口怎么显示，行为时好时坏」问题，不是 React 事件问题。

上游还开着：[wry#637](https://github.com/tauri-apps/wry/issues/637)、[tauri#6781](https://github.com/tauri-apps/tauri/issues/6781)、[tauri#4316](https://github.com/tauri-apps/tauri/issues/4316)，wry 0.56 也没修。

真正干活的是 `first_mouse.rs`：遍历 webview 的 NSView 子树，按公开类关系找到 `WKWebView` 实例，用 objc runtime 把它所属的 `WryWebView` 类的 `acceptsFirstMouse:` 换成恒 YES。启动时在 `setup` 主线程里对两个窗口**同步补完再 show**——顺序不能反：先显示再用 `run_on_main_thread` 异步排队，用户点得快就能赶在补丁前，第一次仍被吞。后续每次 `show()` / `present()` 再做防御性重补；类方法替换幂等。

- 不依赖 WebKit 私有子视图名；若 macOS / wry 改了宿主层级就**静默失效**、退回点两下的老行为，不会崩。配置里的 `acceptFirstMouse` 保留着，上游修好后两边指向同一行为。
- 代价是激活那一下的点击会真生效（手正好落在「清空历史」上就真清了），对划词工具这笔账划算。

**验证只能用真实的 `CGEventPost` 点击**，AppleScript 的 `click at` 走 AX、绕过整个 first-mouse 行为，测不出来。而 `CGEventPost` 要求**发事件的进程**（终端 / IDE）有辅助功能权限，没授权的话事件被系统静默丢弃、看起来像「修了没用」——先点一下标题栏自检工具是否真在发事件。没权限就退回手点，同时用 `CGWindowListCopyWindowInfo` 拉窗口栈看设置窗口（780x620）是在第几次点击后出现的，这比肉眼判断可靠。

### 输入法吞掉的那一下（和上面是两回事）

「第一下没反应」有**两个**独立成因，只修一个仍然会被报 bug。上面那个是 app 未激活；这个是 app 已经在前台、输入法处于组词态：

拼音还没上屏时，落到 webview 上的第一次 mousedown 被输入法拿去结束组词，**不转发**给 WKWebView。web 层只收到一个孤儿 `mouseup`，浏览器不会凭 mouseup 合成 click，React 的 onClick 于是不触发。实测（Rime / 鼠鬚管）第一下的完整事件序列只有：

```
compositionend  →  mouseup(button[设置])          ← 没有 pointerdown / mousedown / click
```

第二下才是 `pointerdown → mousedown → mouseup → click`。**两个窗口的所有控件都中招**，不止设置齿轮：设置页里「输入框打完字直接点保存」是同一个坑。

治法在 `src/lib/imeClick.ts`，`main.tsx` 里对两个窗口统一装：前面没有配对 mousedown 的孤儿 mouseup，就是被吞掉的那一下，给它补一个 click（浏览器自己补了就不重复派发；落点是文本框时顺手 `focus()`，还原原生的 mousedown → 聚焦 → click 顺序）。

复现要三件事同时成立，缺一个就复现不出来、白查半天：

- 输入法真的在组词（有 marked text），只是「输入框有焦点」不够；
- 打字必须用 `CGEvent` 真键码，**AppleScript 的 `keystroke` 绕过输入法**，根本不会组词；
- 输入源要显式选定（`TISSelectInputSource`），macOS 会按窗口记住输入源，手动切过就不可信了。

划词的载荷经 `windows::present()` 送到主窗口。冷启动时事件可能早于 React 挂载，所以 `state.rs` 里留了一份暂存，`Main` 挂载时主动 `takePendingLookup()` 兜一次。

流式事件用 `app.emit()` **广播**，前端按 `streamId` 分发。

## 历史存储

`history.rs`，SQLite 在 `~/Library/Application Support/Glossa/history.db`。

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

两种形式都带 `identifier: com.github.glossa`，PopClip 据此认出是同一个扩展——改端口后重装会覆盖，而不是多出一个重复图标。

一键安装依赖 macOS 文件关联，Setapp 版、多版本共存、关联被别的软件抢走都可能失效，所以**手动路径必须保留**。`open` 之前先探测 PopClip 是否存在，否则 macOS 会弹「没有可打开此文件的应用」这种让人摸不着头脑的错误。

## 关键约定

**API key 不进 webview。** 所有 LLM HTTP 调用都在 Rust 侧完成，前端只通过 `invoke` 触发、通过事件收增量。`test_provider` / `list_models` 是例外——它们由设置页传入完整 `Provider`（含 key），因为此时 key 本来就是用户正在这个表单里输入的。

**明文存 key 是用户明确选择的方案**，不是疏漏。文件权限收紧到 0600，README 里有安全说明。不要擅自改回 Keychain。

**改 `APP_DIR_NAME` 就是断老用户的配置。** 数据目录名（`config.rs` 的 `APP_DIR_NAME`）不只是历史，settings.json 里还有 API key——换个名字对老用户等于配置全丢。项目从 EnAssistant 改名时靠 `migrate_legacy_dir()` 把旧目录整体 rename 过来，它必须排在 `config::load` 和 `History::open` 之前调（`lib.rs` 的 `setup`），否则新目录先被创建出来，迁移条件不成立。真要再改名，照这个模式加一层，别直接换常量。

**结构化输出不用 `response_format` / tool use。** 多数 OpenAI 兼容端点（Ollama、LM Studio、各类中转）不支持或行为不一致。释义靠提示词约束 JSON，前端 `jsonish.ts` 做容错增量解析。这是跨协议唯一稳的做法，改用原生结构化输出会让一半端点挂掉。

**用本地端口而不是 `glossa://` deep link。** macOS 不支持运行时注册 URL scheme，deep link 只有装到 `/Applications` 的打包 .app 才能测，`tauri dev` 下无法调试。另外 PopClip 的 url action 会顺带打开浏览器标签页，所以片段用的是 shell script action。

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

### 推理模型

`Delta` 分 `Text` 与 `Reasoning` 两个变体，别合成一个：思考内容混进正文，`parsePartialJson` 拿到的就是一大段自然语言。

- **思考 token 也算 completion token**。上限原来是 800，`deepseek-v4-flash` 光思考就写满 800、`finish_reason: "length"`、正文零字符——落库落进一条空释义，界面一片空白且**不报错**。现在默认 4000（实测最坏 692，余量 ~6 倍）。
- 上限是 **provider 级设置**（`Provider::max_tokens`，界面上「输出上限」），常量只是默认值。它是端点能力不是全局偏好：支持 64K 输出的 reasoner 和上限 4096 的中转要的值正相反。**默认值别往 4096 以上调**——那是相当一部分端点的输出硬上限，超了直接 HTTP 400，症状是「换个 provider 就全查不出来」。要更大让用户自己在该 provider 上填。
- 低于 `MIN_MAX_TOKENS`（256）的值当没填：输入框清空会送来 0，照用的话思考还没写完就截断，正文永远是空的。
- openai 协议思考在 `delta.reasoning_content`（此时 `delta.content` 是 `null`）；anthropic 协议在 `content_block_delta` 的 `delta.thinking`。
- **只认 `content` 会漏掉整个思考流**，长思考期间界面完全没动静，看起来像卡死。前端 `ThinkingNote` 就是给这段时间用的，正文一到就撤掉。
- 两道兜底都要留着：`openai.rs` 在「没有正文 + `finish_reason == "length"`」时报错；`spawn_stream` 在 `full` 为空时发 `Error` 而不是 `Done`，且**不落库**。静默的空卡片是最难排查的失败方式。
- 收流的地方（`collect_text`、forwarder）**必须收到 channel 关闭为止**，且要匹配全部变体。写成 `while let Some(Delta::Text(t))` 的话，第一个思考增量就会让循环提前退出。

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

### 什么时候 dev 不够用

阶段一整套功能都能在 `tauri dev` 下验完——取词走 PopClip + 本地 HTTP，完全不碰系统权限。

阶段二 `capture/` 落地后不成立了：`tauri dev` 跑的是 `target/debug/Glossa` 裸二进制，TCC 对未签名/ad-hoc 二进制按**路径 + cdhash** 记账，每次改 Rust 重编 hash 就变、辅助功能授权随之失效（详见上面 AX 那节的坑列表）。分三层处理：

1. **纯逻辑走 `cargo test`** — UTF-16 range 切片、`context.rs` 的前后文截取都是纯函数，不需要真机权限，别为了跑它们去 build。
2. **AX 真机行为在 `npm run tauri build` 产物上验** — 产物 `.app` 放固定路径（`/Applications` 或一个不会挪的目录）再授权，之后反复启动都算同一个 app。不要在 `target/debug/` 里授权。
3. **反复要权限烦到不行时才上签名** — 正常签名的 app 按 designated requirement（签名 identity + bundle id）记账，重编重签后授权保留。搞一个自签名 code signing 证书，给 dev 产物签固定 identity + 固定 bundle id。

配套细节：重新授权前先去系统设置 → 隐私与安全性 → 辅助功能把旧条目**删掉再重加**。陈旧条目会出现「勾是勾着的但权限没生效」，这个状态极易误判成取词代码写错了。

## 尚未实现

生词本、复习（FSRS）、本地词典即时层、结果缓存。都在阶段二之后，见 README 的路线图。
