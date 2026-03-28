# YouClaw

让 Agent 真正驻留在你的桌面里。

YouClaw 是一个本地优先的桌面 Agent 工作台，围绕会话、工具调用、审批、工作区和持久化上下文来组织 AI 协作。

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
