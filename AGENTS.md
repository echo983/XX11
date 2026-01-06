# 开发指南与规范

## 项目架构
- `src/main.rs`: 入口。
- `src/orchestrator.rs`: 核心调度，管理视觉反馈循环 (Iteration Loop)。
- `src/llm/gpt52.rs`: 模型驱动，处理异构模型请求、Schema 生成与缓存键管理。
- `src/x11/`: 底层渲染与事件。
    - `renderer.rs`: 离屏渲染引擎，支持 TTF 与位图合成。
- `prompts/`: 外置提示词库。

## 代码规范
- **Schema 优先**：任何指令变更必须先同步 `src/llm/gpt52.rs` 中的 JSON Schema。
- **离屏渲染一致性**：评估用的 JPG 图片必须由 `renderer::render_to_buffer` 生成，确保 LLM 看到的与用户看到的一致。
- **缓存敏感**：在 `User` 消息中，必须保持 `(静态指令) -> (动态图片) -> (动态代码)` 的顺序以保护 Cache Prefix。

## 调试建议
- 分析布局失败原因时，优先检查 `debug_out/iter_X_reason.txt`。
- 如果超时，检查 `debug_out/iter_X_draft.jpg` 是否因尺寸过大导致传输慢（当前缩放比 0.3）。
