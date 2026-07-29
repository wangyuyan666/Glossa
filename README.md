# EnAssistant

macOS 上的 LLM 英语学习工具。在任意 app 里划中不认识的词，主窗口给出**结合上下文**的释义，可以继续追问，查过的词留在历史里。

和普通词典的区别在于上下文：词典给的是这个词的全部义项，EnAssistant 给的是「这句话里它是什么意思」。

> 当前是**阶段一**。取词层借用 PopClip，因此拿不到选中词的上下文，释义会降级为「最常见义项」。真正的上下文能力在阶段二自建取词层后落地，见 [路线图](#路线图)。

## 环境要求

- macOS
- [PopClip](https://www.popclip.app/)（阶段一的取词层）
- Node.js 20+、Rust 1.85+

## 运行

```bash
npm install
npm run tauri dev
```

首次启动没有任何模型配置，设置窗口会自动打开。

打包：

```bash
npm run tauri build
```

## 配置模型服务

设置窗口 → **模型服务** → 添加服务。每个服务四个字段：

| 字段 | 说明 |
| --- | --- |
| 名称 | 自己认得就行 |
| 协议 | `OpenAI 兼容` 或 `Anthropic` |
| base_url | 端点地址，结尾带不带 `/v1` 都能识别 |
| api_key | 明文保存，见[安全说明](#安全说明) |

**协议怎么选**：

- `OpenAI 兼容` — OpenAI 官方、DeepSeek、硅基流动、OpenRouter、Groq、Ollama、LM Studio 等，走 `/chat/completions`
- `Anthropic` — Anthropic 官方及其兼容代理，走 `/v1/messages`

常见 base_url：

```
https://api.openai.com/v1
https://api.deepseek.com/v1
https://api.anthropic.com
http://127.0.0.1:11434/v1      # Ollama
http://127.0.0.1:1234/v1       # LM Studio
```

填好后点 **拉取模型列表** 自动补全模型名（部分兼容端点没实现该接口，拉不到就手填），再点 **测试连接** 验证。

### 角色绑定

两个角色可以指向不同厂商：

- **释义** — 划词后出释义。走结构化输出，求快求便宜，用小模型即可
- **对话** — 追问时使用，求强

## 配置 PopClip 取词

设置窗口 → **取词** → 点「安装到 PopClip」。EnAssistant 会生成扩展并交给 PopClip，在它弹出的确认框里点安装即可。

安装后：在任意 app 里选中英文 → PopClip 条上点 EnAssistant 图标 → 主窗口唤起并开始查询。

### 手动安装

一键安装依赖 macOS 的文件关联。Setapp 版 PopClip、多版本共存、关联被别的软件抢走都可能让它失效，这时走手动路径：

设置窗口 → **取词** → **手动安装** → 复制片段 → 粘贴到任意能选中文本的地方 → **选中整段** → PopClip 条上出现 **Install Extension** → 点它。

片段内容（端口跟着设置里的值走）：

```yaml
#popclip
name: EnAssistant
identifier: com.peter.enassistant
icon: symbol:character.book.closed
interpreter: bash
shell script: curl -s -X POST http://127.0.0.1:8765/lookup --data-urlencode "q=$POPCLIP_TEXT" -o /dev/null
```

> 开头的 `#popclip` 是 PopClip 识别 snippet 的标志，少了它整段就只是普通文本。一键安装用的是另一种形式（`.popclipext` 目录 + `Config.yaml`），那种**不需要**这个头行。

### 没反应的排查

1. EnAssistant 在跑吗——`curl http://127.0.0.1:8765/ping` 应返回 `EnAssistant`
2. 端口对得上吗——改过端口要重启 EnAssistant 并重新安装一次扩展
3. PopClip 里该扩展没被禁用吧

> 为什么用本地端口而不是 `enassistant://` deep link：macOS 不支持运行时注册 URL scheme，deep link 只有装到 `/Applications` 的打包 .app 才能测，`tauri dev` 下没法调试。另外 PopClip 的 url action 会顺带打开浏览器标签页。

## 主窗口

只有主窗口和设置窗口两个界面。划词触发时唤起的也是主窗口，不再有独立弹窗。

左边历史、右边查词：

- 顶部输入框直接敲单词或短语，回车查询
- 左侧列出所有查过的词，倒序。点一条恢复**整个会话**——释义加当时的全部追问，不重新请求模型
- 搜索框按词模糊匹配
- 悬停某条记录出现删除按钮；底部可清空全部

关掉主窗口后点 Dock 图标可以唤回。

> 多桌面（Space）下要注意：划词时若主窗口在别的 Space，macOS 会把你**整个切换过去**。

## 历史存在哪

```
~/Library/Application Support/EnAssistant/history.db
```

SQLite。这是**历史流水**不是词表：同一个词查两次记两条。

## 快捷键

| 键 | 作用 |
| --- | --- |
| `Enter` | 查询 / 发送追问 |

## 安全说明

API key 以**明文** JSON 保存在：

```
~/Library/Application Support/EnAssistant/settings.json
```

文件权限收紧到 `0600`（仅当前用户可读写），但明文落盘意味着：任何能读该文件的进程或用户都能拿到 key，文件也会进入 Time Machine 备份和任何目录同步。介意的话请为 EnAssistant 单独申请一个额度受限的 key。

设置界面里 key 默认以圆点显示，点右侧小眼睛可切换明文。

## 已知限制

- **阶段一拿不到上下文**。PopClip 只传选中的那几个词，不提供所在语句，也不提供来源 app。释义因此退化成常见义项。
- 依赖 PopClip（付费软件）。
- 没有生词本和复习，查过的词不落库。
- 改取词端口后需重启 EnAssistant。

## 路线图

**阶段二：自建取词层**

用 Accessibility API 取 `kAXSelectedTextAttribute`，失败时降级为模拟 `Cmd+C`；同时读 `kAXValue` + `kAXSelectedTextRange`，从整段文本里切出选中词前后各若干字符作为上下文。届时去掉 PopClip 依赖，释义才真正变成「此处含义」。

参考实现：[`yetone/get-selected-text`](https://github.com/yetone/get-selected-text)（Rust）。

**之后**：生词本 + FSRS 复习（考原句填空，不是孤立单词）、本地词典做即时层、结果缓存。

## 开发

```bash
npm run tauri dev     # 开发（前端热更新，Rust 改动会重编）
npm run build         # 前端类型检查 + 构建
npm test              # 前端单测（vitest）
npm run tauri build   # 打包 .app
cd src-tauri && cargo test
```

架构与目录约定见 [AGENTS.md](./AGENTS.md)。
