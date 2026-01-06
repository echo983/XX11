# XX11: LLM-Driven GUI Bridge

一个通过 X11 协议，让 LLM 以绘图原语驱动图形界面并实现视觉自审的 Windows Rust Daemon。

## 核心特性
- **视觉反馈环 (Visual Feedback Loop)**：LLM 生成界面后，系统自动生成草稿截图回传，LLM 自我评估并修正布局（最多迭代 4 次）。
- **异构模型架构**：
  - **逻辑生成**：GPT-5.2 (负责深度逻辑与规划)
  - **视觉校审**：GPT-5-Mini (负责快速、廉价的布局纠错)
- **严格模式 (Structured Outputs)**：使用 JSON Schema 强制保证绘图指令的 100% 格式正确率。
- **缓存优化**：利用 OpenAI Prompt Caching，通过前缀固定极大减少响应延迟和成本。
- **本地渲染**：集成 `fontdue` 与离屏渲染逻辑，支持高质量中文/Emoji 显示。

## 环境要求
- Windows OS
- [VcXsrv](https://sourceforge.net/projects/vcxsrv/) (必须运行，配置为 `127.0.0.1:0.0`, 禁用 access control)
- `OPENAI_API_KEY` 环境变量

## 快速开始
```powershell
# 编译
cargo build

# 运行 (交互模式)
cargo run
```

## 调试模式 (DEBUG)
启用 `$env:AGD_DEBUG="1"` 后：
- **Token 监控**：实时输出 Input/Output/Cached Tokens 数量。
- **过程存档**：所有迭代生成的 JSON、草稿图 (JPG) 和 LLM 拒绝理由都会保存到 `debug_out/` 目录。
```powershell
$env:AGD_DEBUG="1"; cargo run
```

## DSL 规范 (AGD/0.2)
详细规范见 `初步需求.txt` 与 `prompts/system.txt`。
- **clear**: 清屏。
- **rect**: 矩形/按钮。
- **text**: 标签化文本 (24px)。
- **line**: 逻辑连接线。
- **circle / ellipse / round_rect / arc**
- **polyline / polygon / path**
- **image**
