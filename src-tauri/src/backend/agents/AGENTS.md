# AGENTS.md - backend/agents 模块说明

本文档只约束 `src-tauri/src/backend/agents/` 及其子目录。

目标：说明 agent runtime 的职责边界、执行链路、tools 子系统约定，以及后续修改该模块时应遵守的规则。

## 模块职责

`backend/agents/` 是 YouClaw 的 agent 运行时层，负责：

- 启动和执行一次 turn
- 组装 prompt / message / bootstrap context
- 收集模型流式输出与 tool call
- 调度工具执行并把结果写回对话历史
- 管理 agent workspace 与 memory 检索

它不是：

- provider 基础设施层
- websocket transport 层
- 通用 storage facade
- session / settings 的业务编排层

如果一个能力主要是在“处理 ws 请求”或“维护会话业务规则”，优先放到 `ws/` 或 `services/`，不要塞回 `agents/`。

## 目录概览

- `mod.rs`
  组合入口。暴露 `start_turn` / `spawn_turn`，负责把 turn 执行任务挂到 tokio。
- `turn_start.rs`
  创建 turn、用户消息、初始持久化记录。
- `turn_execution.rs`
  主执行链路。负责加载 provider / session / config，构建工具列表，驱动 step 循环，处理工具调用、结束态和 usage。
- `message_builder.rs`
  把系统提示、历史消息、tool result 组织成模型输入。
- `stream_collector.rs`
  从模型流里抽取 reasoning、text、tool calls，并在需要时注册 runtime tool-call binding。
- `tool_dispatcher.rs`
  顺序执行当前 step 的 tool calls，记录 metrics，发出 `tool.finished` 事件，并把 tool result 回写到消息流。
- `tool_result_processor.rs`
  统一清洗/归一化工具输出。
- `context_compactor.rs`
  压缩上下文，减少 token 占用。
- `summarizer.rs`
  提取摘要文本，服务于上下文压缩。
- `token_estimator.rs`
  估算消息 token。
- `workspace.rs`
  管理 agent 内部上下文目录、`AGENTS.md` 模板安装与系统 prompt 拼装。
- `tools/`
  工具实现与工具运行时。

## 执行链路

一轮 agent turn 的主路径如下：

1. `start_turn` 创建 turn 和用户消息。
2. `spawn_turn` 把执行任务交给后台。
3. `turn_execution::execute_turn` 加载 session/provider/config，构建 messages 和 tools。
4. 模型流由 `stream_collector` 收集，生成 text / reasoning / tool calls。
5. 如果 step 中存在 tool call，交给 `tool_dispatcher::handle_tool_calls` 顺序执行。
6. tool result 被持久化、写入消息流，并进入下一轮 step。
7. 没有更多 tool call 时，turn 正常结束；取消或失败时由 `mod.rs` 统一发出 turn 级事件。

## Tools 子系统规则

### 当前边界

`tools/` 下的分工如下：

- `tool_runtime.rs`
  运行时共享上下文、tool-call claim、审批决策与等待。
- `filesystem_context.rs`
  文件系统工具的共享 helper：路径校验、diff 预览、文本读写。
- `bash.rs` / `write_file.rs` / `edit_file.rs` / `read_*` / `search_*`
  具体工具实现。
- `tools/mod.rs`
  工具注册、builder 聚合、tool action 映射。

### 审批边界

当前审批边界已经统一收敛到 `tool_runtime.rs`：

- 工具只负责提供审批规格：
  - `ToolApprovalRequest`
  - `action`
  - `subject`
  - `preview_json`
  - `ToolApprovalMode`
- runtime 负责审批决策：
  - 是否需要审批
  - `full_access` 是否直接放行
  - 等待审批结果
  - 发出 `chat.step.tool.requested`

不要在具体工具里重复做以下事情：

- 手工调用 `new_tool_approval(...)`
- 直接调用 `await_approval(...)`
- 自己判断 `SessionApprovalMode::FullAccess`

正确做法是：

- 工具构造 `ToolApprovalRequest`
- 调用 `context.runtime.authorize_tool_call(...)`
- 根据 `ToolApprovalOutcome` 执行后续逻辑

### 哪些语义仍然留在工具里

审批策略统一在 runtime，但工具自己的业务语义仍应保留在工具内，例如：

- `dry_run` 只预览不落盘
- shell 风险标记 `risk_flags`
- diff / preview 的具体组织方式

也就是说：

- policy 在 runtime
- payload 在 tool
- effect 在 tool

## 新增或修改工具时的要求

1. 先判断该能力是否真的属于 agent tool。
   如果更像 session service、provider helper、storage query，不要放进 `tools/`。

2. 新工具默认要回答三个问题：
   - 输入如何校验
   - 输出如何结构化
   - 是否需要审批，以及审批元数据是什么

3. 有副作用的工具遵循统一模式：
   - claim tool call
   - 校验输入
   - 构造 `ToolApprovalRequest`
   - 调用 `authorize_tool_call`
   - 获批后执行副作用
   - 记录 storage / metrics / output

4. 如果工具需要与模型原始 `ToolCall` 一一绑定：
   在 `tools/mod.rs` 的 `requires_tool_call_binding(...)` 中登记。

5. 不要把通用逻辑复制到多个工具文件。
   能抽到 `filesystem_context.rs` 或 `tool_runtime.rs` 的，优先抽共享 helper。

## Context / Profile / Memory 边界

- `workspace.rs` 只处理 agent 内部上下文目录、`AGENTS.md` 模板安装、系统 prompt 拼装。
- `profile` 能力负责 `user` / `soul` 两类每轮注入的持久画像，不走文件系统。
- `memory_system_*` 负责按需检索的长期记忆，不默认整库注入 prompt。
- 工具如果要改动 `user` / `soul`，必须使用 `profile_update`；如果要改动长期记忆，必须使用 `memory_system_*`，不要再引入文件式记忆规则。

## 修改约束

- 不要把 `tool_dispatcher.rs` 重新变成“每个工具各自写一套策略”的入口。
- 不要在 `storage/` 里实现 agent 流程控制。
- 不要让 `ws/` 直接理解具体工具的审批细节。
- 不要把 prompt/workspace 逻辑散回 `services/` 或 `ws/`。

## 验证建议

修改本目录后，默认至少执行：

```bash
cd src-tauri
cargo test backend::agents::tools::
cargo check
```

如果改动了 step 流程、消息构造、上下文压缩，建议再补充对应单测。
