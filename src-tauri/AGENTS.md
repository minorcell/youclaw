# AGENTS.md - src-tauri 子树说明

本文档约束 `src-tauri/` 及其子目录。

目标：为 Tauri 桌面端与 Rust 后端提供统一的模块边界、修改约束和最小验证规则。

## 作用范围

本文件覆盖：

- `src-tauri/src/lib.rs`
- `src-tauri/src/backend/**`
- `src-tauri/capabilities/**`
- `src-tauri/tauri.conf.json`

如果子目录下存在更近的 `AGENTS.md`，优先遵循更近的那份。

当前已存在的更细粒度文档：

- [backend/agents/AGENTS.md](/Users/mcell/Desktop/agents/rs-agent/youclaw/src-tauri/src/backend/agents/AGENTS.md)

## 顶层职责

`src-tauri/` 是桌面壳和 Rust 后端运行时，不负责前端页面渲染。

它的主要职责：

- 启动 Tauri 应用
- 初始化 `BackendState`
- 启动本地 ws 服务
- 提供后端 domain/service/storage/provider/agent runtime
- 处理桌面端能力配置

它不应该承担：

- React 页面状态或渲染逻辑
- 前端组件样式决策
- 与 UI 强耦合的临时业务拼装

## 目录边界

- `src/lib.rs`
  Tauri 入口。只负责应用启动、插件注册、桌面状态挂载、启动 ws 服务。
  不要把具体业务流程继续堆到这里。

- `src/backend/mod.rs`
  后端组合根。负责组装 `storage`、`workspace`、`ws_hub`、`approvals` 与各类 service factory。
  不要在这里持续增加具体业务方法。

- `src/backend/ws/`
  只处理 websocket 请求分发、请求/响应 envelope、turn 事件出口。
  不直接承载复杂业务规则。

- `src/backend/services/`
  application service 层。负责编排 session/provider/workspace/runtime 相关用例。
  service 应保持显式 struct，不要重新退化成在 `BackendState` 上堆大量方法。

- `src/backend/storage/`
  持久化与查询层。只做 sqlite 读写、索引与聚合查询。
  不在这里写 turn 流程、审批等待、ws 事件发布。

- `src/backend/providers/`
  provider 协议、endpoint、鉴权和 client helper。
  不放 session 业务规则。

- `src/backend/agents/`
  agent runtime 层：turn 执行、workspace、memory、tool system。
  该目录的进一步规则见下级文档：
  [backend/agents/AGENTS.md](/Users/mcell/Desktop/agents/rs-agent/youclaw/src-tauri/src/backend/agents/AGENTS.md)

- `src/backend/models/`
  类型边界层。优先按 `domain / requests / events / responses` 语义使用，不要继续平铺导入一切类型。

- `capabilities/`
  Tauri capability 配置。
  修改权限时先确认是否真的是桌面能力问题，而不是应用内部审批逻辑问题。

- `tauri.conf.json`
  窗口、打包、前端入口、titlebar 等桌面配置。
  窗口行为问题优先先看这里，再决定是否是前端布局问题。

## 修改原则

1. 先判断改动属于哪一层，再落文件。
   不允许“为了方便”把 service、storage、agent runtime 混写在一起。

2. 桌面能力问题先区分两类：
   - Tauri 原生配置问题
   - 应用层业务/布局问题

3. 后端逻辑优先保持单向依赖：
   - `ws -> services -> agents/providers/storage/models`
   - `agents -> providers/storage/models`
   - `storage` 不反向依赖上层流程

4. 新代码默认优先强类型，不要随意退回裸 `serde_json::Value`。
   如果某处必须动态，至少保证结构可辨识。

5. 不要在 `src/lib.rs` 或 `backend/mod.rs` 做“临时修补式”业务扩展。
   组合根只负责组装，不负责承载业务。

## 审批与工具边界

当前工具审批已经统一收敛在 `backend/agents/tools/tool_runtime.rs`。

因此：

- 不要把工具审批逻辑塞进 `ws/`
- 不要把 session 权限判断散落回各个具体工具
- 不要把审批状态机塞进 `storage/`

如果要改动 agent 工具系统，先读：

- [backend/agents/AGENTS.md](/Users/mcell/Desktop/agents/rs-agent/youclaw/src-tauri/src/backend/agents/AGENTS.md)

## 常见修改落点

- provider 配置或 endpoint 归一化：`src/backend/providers/`
- 会话绑定、审批模式、归档恢复：`src/backend/services/` + `src/backend/ws/`
- turn 执行、tool call、workspace prompt：`src/backend/agents/`
- sqlite schema / 查询 / usage / approval record：`src/backend/storage/`
- 窗口拖拽、titlebar overlay、capability：`tauri.conf.json`、`src/lib.rs`、前端布局联合排查

## 最小验证

修改 `src-tauri/` 后默认至少执行：

```bash
cd src-tauri
cargo check
```

如果改动涉及 agent tools、审批、workspace、memory，再执行：

```bash
cd src-tauri
cargo test backend::agents::tools::
```

如果改动涉及 schema、storage 或业务边界较大的 service 调整，补充对应测试，而不是只依赖 `cargo check`。
