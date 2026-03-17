# YouClaw

让 Agent 真正驻留在你的桌面里。

YouClaw 是一个本地优先的桌面 Agent 工作台，围绕会话、工具调用、审批、记忆和工作区文件来组织 AI 协作。

## 现在有什么

- 本地桌面应用：Tauri 2 + React 19 + Rust
- Chat + timeline：查看 step、tool call、审批和结果
- Workspace：把 `AGENTS.md`、`MEMORY.md`、`memory/*.md` 作为可编辑上下文
- Memory：本地索引、显式 `search -> get` 召回
- Tooling：文件读写、搜索、bash 执行、权限模式切换

## 开发

```bash
pnpm install
pnpm tauri dev
```

## 常用命令

```bash
pnpm dev
pnpm build
pnpm tauri build
npx tsc --noEmit
cd src-tauri && cargo check
```

## 技术栈

- Frontend: React 19, TypeScript, Vite, Zustand, Tailwind CSS v4
- Desktop: Tauri 2
- Backend: Rust, axum WebSocket, rusqlite

## License

MIT
