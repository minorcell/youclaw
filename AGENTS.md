# AGENTS.md - Agent 编码指南

本文档为在此代码库工作的 AI agent 提供统一规范。目标是：简约、一致、可维护、低开销。

## 项目概述

- **名称**: YouClaw
- **技术栈**: Tauri 2 + React 19 + TypeScript + Rust
- **前端**: React 19, react-router-dom, zustand, shadcn-style UI, Tailwind CSS v4
- **后端**: Rust + axum WebSocket 服务器, rusqlite
- **Agent 运行时**: 本地 aquaregia 依赖

## 常用命令

### 前端 (React/TypeScript)

```bash
pnpm install
pnpm dev
pnpm build
pnpm preview
```

### 桌面应用 (Tauri)

```bash
pnpm tauri dev
pnpm tauri build
```

### 后端 (Rust)

```bash
cd src-tauri
cargo check
cargo build --release
cargo test
cargo test <test_name>
cargo test --test <test_file_name>
```

### 轻量校验（默认优先）

```bash
npx tsc --noEmit
cd src-tauri && cargo check
```

## 核心原则

1. **简约至上**
   - 非必要不使用边框与大间距。
   - 优先通过留白、层级、对比来表达结构。

2. **主题优先**
   - 尽可能使用系统主题变量和主题系统。
   - 避免硬编码颜色（hex/rgb/固定色类名），状态色也优先 token 化。

3. **组件优先复用**
   - 优先使用 shadcn/ui 组件，避免重复造基础组件。

4. **低开销验证**
   - 非必要不要每次完整构建。
   - 默认使用轻量检查（`tsc --noEmit`、`cargo check`）。
   - 仅在发布前、验证构建链路、或打包行为变更时执行完整构建。

5. **发现能力**
   - 专注于做用户的需求，不要分散注意力。
   - 发现Bug、可优化点时，用户同意之后优先在 Github 创建 issue 记录问题。

## 工程方法论

我们默认采用：**轻量分层 + 有界上下文 + 渐进重构**

1. **先定边界，再写实现**
   - 先判断能力属于哪个上下文、哪一层，再决定文件与目录归属。
   - 不允许“先写完再随便找个地方塞进去”。

2. **依赖单向，职责单一**
   - 上层编排下层，下层不反向耦合上层。
   - 一个模块只负责一种语义，不同时承担 transport、业务编排、持久化、视图组装中的多种职责。

3. **显式依赖，避免隐式中心对象**
   - 优先显式 service / context struct，不把业务能力继续堆回全局状态对象。
   - 一个能力只能有一个明确归属，禁止目录语义重复。

4. **渐进重构，持续验证**
   - 先搬边界，再收实现，避免大爆炸式重写。
   - 每轮重构至少跑最小验证，确保边界收敛不带来行为回退。

具体目录职责、依赖方向、类型分层和重构要求，以下方“架构规范”为准。

## 架构规范

以下规范比“代码风格”优先级更高。出现冲突时，先满足架构边界，再讨论实现细节。

### 后端目录职责

- `src-tauri/src/backend/mod.rs`
  - 只作为组合根和共享运行时容器。
  - 负责初始化 `storage`、`workspace`、`ws_hub`、`approvals`、service 构造入口。
  - 不继续承载具体业务流程。

- `src-tauri/src/backend/ws/`
  - 只处理 WebSocket transport：路由分发、payload 解码、response 包装。
  - 不直接编排复杂业务，不直接拼装运行时对象。

- `src-tauri/src/backend/services/`
  - 作为 application service 层。
  - 负责用例编排、跨模块协调、事件发布。
  - 优先使用显式 struct 注入依赖，禁止回到 `impl BackendState` 大量挂方法的模式。

- `src-tauri/src/backend/agents/`
  - 只放 agent runtime 相关能力：turn 执行、prompt、tool runtime、workspace、memory。
  - 不放 provider 基础设施和通用持久化细节。

- `src-tauri/src/backend/providers/`
  - 只放 provider 协议、endpoint 归一化、鉴权、client helper。
  - 不放 session/chat 业务规则。

- `src-tauri/src/backend/storage/`
  - 只放持久化与查询实现。
  - 不写 websocket 事件发布，不写 agent 流程控制，不写页面/交互语义。

- `src-tauri/src/backend/models/`
  - `domain/`：领域实体、工厂、核心枚举。
  - `requests/`：请求 DTO。
  - `events/`：ws event payload。
  - `responses/`：普通响应 payload。
  - 顶层 `models/mod.rs` 目前仅作为迁移期兼容出口，新代码不应优先依赖它。

### 后端依赖方向

- 默认方向：`ws -> services -> domain/infrastructure`
- `services` 可以依赖 `agents`、`providers`、`storage`，但应只做编排，不吞并实现细节。
- `agents` 可以依赖 `domain`、`events`、`responses`、`providers`、`storage`。
- `storage` 不依赖 `ws`、`services`、`agents` 业务流程。
- 如果一个模块需要反向依赖上层模块，先停下来重审边界，而不是直接引入。

### 类型使用规则

- 新代码默认显式从以下命名空间导入：
  - `models::domain::*`
  - `models::requests::*`
  - `models::events::*`
  - `models::responses::*`
- 不要在新代码里继续把所有类型都从 `models::*` 平铺导入。
- 请求 DTO 不要和响应 payload、事件 payload、领域实体放在同一个文件。
- 能强类型表达的结构不要退化成裸 `Value`；如果必须动态，必须带可辨识 `kind`。

### Service 规则

- service 必须是显式 struct，不继续扩展 `BackendState` 作为隐式 service 容器。
- service 负责协调，不负责保存全局状态副本。
- 同一个事件的发布逻辑要集中，不要在多个模块里各写一份同名事件拼装。
- 如果两个 service 同时维护同一种业务规则，应抽出共享 helper 或重新划分归属。

### Storage 规则

- `storage/mod.rs` 仅允许作为聚合入口，不应持续膨胀为单一超大 facade。
- 新增查询或写入时，优先放入对应上下文文件，如 `sessions.rs`、`usage.rs`、`memory.rs`、`shell.rs`。
- 不要把“业务校验 + 存储写入 + 事件通知”混在同一个 storage 方法里。

### 前端架构规则

- `src/pages/` 只做页面组装，不承担复杂状态推导和协议解释。
- `src/features/` 或页面下的 `hooks/`、`adapters/`、`components/` 负责拆分页面逻辑。
- `src/store/` 只做状态容器与 reducer，不直接承担页面渲染策略。
- 前端 view model、render unit、timeline adapter 不要再塞回通用 `lib/types.ts`。

### 重构落地规则

- 优先级顺序：先收边界，再收命名，再收内部实现。
- 大重构默认分多步提交：目录归属、调用面切换、内部清理分开做。
- 迁移期允许兼容出口存在，但必须在文档或注释里明确标出“transitional”。
- 每轮重构结束至少跑一轮最小验证：
  - 前端：`npx tsc --noEmit`
  - 后端：`cargo check`
  - 行为或模型边界调整较大时，再跑 `cargo test`

## TypeScript / React 规范

1. **导入规范**
   - 使用路径别名 `@/`（如 `@/components`, `@/lib`, `@/store`, `@/pages`）。
   - 导入顺序：外部库 -> 内部模块 -> 类型。
   - 类型导入使用 `type`。

2. **命名规范**
   - 组件、类型：PascalCase。
   - 函数、变量、文件：camelCase（页面目录可使用 `-`）。
   - 布尔值使用 `is`、`has`、`should` 前缀。

3. **类型与错误处理**
   - 使用 strict 模式思维编写代码。
   - 避免 `any`，未知类型使用 `unknown`。
   - 使用 `Extract` 等工具类型处理可辨识联合。
   - 异步函数使用 `try/catch`。
   - fire-and-forget 调用使用 `void` 前缀。
   - 处理 `unknown` 错误时显式转换（如 `String(error)`）。

4. **状态管理与性能**
   - 全局状态使用 Zustand，状态文件放在 `src/store/`。
   - 使用 selector 降低不必要渲染。
   - 大列表优先虚拟化（`virtua` / `react-window`）。
   - 高频更新状态优先 `useRef` 或局部状态。
   - 避免在 render 路径做昂贵计算，必要时使用 `useMemo` / `useCallback`。

5. **组件设计**
   - 保持单一职责，复杂逻辑抽到 hooks 或 utils。
   - 每个组件文件尽量不超过 500 行，超出需拆分。
   - 大组件按功能分区（如 `// --- render sections ---`）。

6. **常量与配置**
   - 禁止魔法数字和硬编码阈值/超时。
   - 使用命名常量或集中配置管理。
   - 应用级常量放在 `src/lib/constants.ts`。

7. **样式约定**
   - 使用 Tailwind CSS v4 + `@tailwindcss/vite`。
   - 使用 `cn()` 组合条件类。
   - 保持与现有 shadcn 风格一致。

## Rust 规范

1. **命名**
   - 变量、函数、模块：snake_case。
   - 类型、trait：PascalCase。
   - 常量：SCREAMING_SNAKE_CASE。

2. **错误处理**
   - 使用 `thiserror` 定义错误类型。
   - 使用 `?` 传播可处理错误。
   - 使用 `.map_err(|e| ...)` 包装并补充上下文。

3. **异步**
   - 使用 `tokio` 作为异步运行时。
   - 异步函数使用 `async fn`。
   - `block_on` 仅用于同步入口。

4. **依赖**
   - 使用 `serde` derive 处理序列化。
   - 使用 `axum` 构建 WebSocket 服务器。
   - 使用 `rusqlite`（bundled feature）操作 SQLite。

## 通用模式

1. **文件组织**
   - 相关文件按目录聚合。

2. **副作用清理**
   - 在 `useEffect` 中返回清理函数取消订阅。
   - 使用 disposed 标志避免组件卸载后状态更新。

3. **WebSocket 约定**
   - 覆盖连接状态：idle / connecting / open / closed / error。
   - 实现指数退避重连策略。
