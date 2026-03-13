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
