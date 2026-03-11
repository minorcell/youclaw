# AGENTS.md - Agent 编码指南

本文档为在此代码库工作的 AI agent 提供编码规范。

## 项目概述

- **名称**: YouClaw
- **技术栈**: Tauri 2 + React 19 + TypeScript + Rust
- **前端**: React 19, react-router-dom, zustand, shadcn-style UI, Tailwind CSS v4
- **后端**: Rust + axum WebSocket 服务器, rusqlite
- **Agent 运行时**: 本地 aquaregia 依赖

## 构建命令

### 前端 (React/TypeScript)

```bash
# 安装依赖
pnpm install

# 启动前端开发服务器
pnpm dev

# 构建前端 (运行 tsc + vite build)
pnpm build

# 预览生产构建
pnpm preview
```

### 桌面应用 (Tauri)

```bash
# 启动 Tauri 开发模式 (前端 + 桌面外壳)
pnpm tauri dev

# 构建 Tauri 应用
pnpm tauri build
```

### 后端 (Rust)

```bash
cd src-tauri

# 检查 Rust 代码 (不构建)
cargo check

# 构建 release 版本
cargo build --release

# 运行测试
cargo test

# 运行单个测试
cargo test <test_name>

# 运行指定文件中的测试
cargo test --test <test_file_name>
```

### 类型检查

```bash
# TypeScript 类型检查
pnpm build  # 运行 tsc && vite build

# 或直接运行
npx tsc --noEmit
```

## 代码风格规范

### TypeScript/React

1. **导入**
   - 使用路径别名 `@/` 进行内部导入 (如 `@/components`, `@/lib`, `@/store`, `@/pages`)
   - 顺序: 外部库 → 内部模块 → 类型
   - 使用 `type` 关键字进行类型导入

2. **组件**
   - 使用函数式组件 + TypeScript
   - 优先使用显式 prop 类型标注而非类型推断
   - 使用 `cva` (class-variance-authority) 处理组件变体
   - 使用 `cn` 工具 (tailwind-merge + clsx) 组合 class
   - UI 风格保持主题一致，减少线条的使用。

3. **命名**
   - 组件和类型使用 PascalCase
   - 函数、变量和文件使用 camelCase (页面文件夹使用 `-`)
   - 布尔变量使用 `is`, `has`, `should` 等前缀

4. **状态管理**
   - 使用 Zustand 管理全局状态
   - 状态文件放在 `src/store/`

5. **错误处理**
   - async 函数使用 try/catch
   - fire-and-forget 的异步调用使用 `void` 前缀
   - 处理 `unknown` 类型错误时进行显式类型转换 (如 `String(error)`)

6. **类型**
   - tsconfig 启用 strict 模式
   - 避免使用 `any`, 不确定类型时使用 `unknown`
   - 使用 `Extract` 工具类型处理可辨识联合

7. **常量与魔法数字**
   - 禁止使用魔法数字 (如 `if (status === 3)`)
   - 禁止硬编码配置值 (如超时、阈值、延迟等)
   - 使用命名常量或配置文件集中管理可配置值
   - 创建 `src/lib/constants.ts` 存放应用级常量

8. **组件原则**
   - 优先使用 shadcn/ui 组件，避免手写基础 UI 组件
   - 每个组件文件不超过 500 行，超出则进行模块化拆分
   - 保持组件单一职责，复杂逻辑抽取到 custom hooks 或 utils
   - 大型组件内部按功能分区 (如 `// --- render sections ---`)

9. **性能与状态**
   - 合理使用 Zustand selectors 避免不必要渲染
   - 大列表使用虚拟化 (如 `virtua` 或 `react-window`)
   - 频繁更新的状态考虑使用 `useRef` 或局部状态
   - 避免在 render 路径中进行昂贵计算，必要时使用 `useMemo`/`useCallback`

### Rust

1. **命名**
   - 变量、函数、模块使用 snake_case
   - 类型和 trait 使用 PascalCase
   - 常量使用 SCREAMING_SNAKE_CASE

2. **错误处理**
   - 使用 `thiserror` 定义错误类型
   - 使用 `?` 操作符传播可处理错误
   - 使用 `.map_err(|e| ...)` 包装错误并添加上下文

3. **异步**
   - 使用 `tokio` 作为异步运行时
   - 异步函数使用 `async fn`
   - 谨慎使用 `block_on` (仅在同步入口点使用)

4. **依赖**
   - 使用 `serde` derive 宏进行序列化
   - 使用 `axum` 构建 WebSocket 服务器
   - 使用 `rusqlite` (bundled feature) 操作 SQLite

### Tailwind CSS

- 使用 Tailwind CSS v4 配合 `@tailwindcss/vite`
- 使用 `cn()` 工具处理条件类
- 遵循 shadcn/ui 组件样式模式

### 通用模式

1. **文件**
   - 每个文件一个主要导出
   - 使用 `index.ts` 做桶导出
   - 相关文件按目录分组

2. **清理函数**
   - 使用 `useEffect` 清理函数处理订阅取消
   - 设置 disposed 标志防止组件卸载后状态更新

3. **WebSocket**
   - 处理连接状态 (idle, connecting, open, closed, error)
   - 实现指数退避重连逻辑
