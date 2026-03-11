# YouClaw

本项目是一个基于 `Tauri + React + TypeScript + Rust` 的本地桌面 Agent MVP。

## 当前能力

- 无登录/注册，启动后直接进入应用
- 首次进入先配置一个 `OpenAI-compatible` provider
- 前端和后端业务通信统一走本地 `WebSocket`
- 多会话 chat
- 一个内置 `filesystem` tool
  - `list_dir`
  - `read_file`
  - `write_file`（需要用户审批）
- 本地持久化
  - `SQLite`：sessions / messages / runs / approvals / file audit
  - `JSON`：provider profiles
  - `JSONL`：run 事件日志
  - `Markdown`：session 快照
- Agent 后端使用本地路径依赖 `/Users/mcell/Desktop/agents/aquaregia`

## 技术栈

- Frontend: `React 19`, `react-router-dom`, `zustand`, `shadcn-style UI`, `Tailwind CSS`
- Desktop shell: `Tauri 2`
- Backend: `Rust`, `axum` WebSocket server, `rusqlite`
- Agent runtime: `aquaregia`

## 开发

### 安装依赖

```bash
pnpm install
```

### 启动前端

```bash
pnpm dev
```

### 启动桌面应用

```bash
pnpm tauri dev
```

### 构建前端

```bash
pnpm build
```

### 检查 Rust 后端

```bash
cd src-tauri
cargo check
cargo test
```

## 运行时数据

应用运行时数据默认写入 Tauri 的 `app_data_dir`。
如果该目录不可用，会回退到当前工作目录下的 `./.youclaw-data/`。

## 说明

当前版本仍是 MVP：

- 只支持 `OpenAI-compatible` provider
- API Key 按需求以明文 JSON 保存
- 文件工具允许全文件系统访问，但写入必须逐次审批
