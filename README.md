# YouClaw

让 Agent 成为你。

## 简介

YouClaw 是一个基于 Tauri + React 构建的桌面应用，旨在让 AI Agent 更好地辅助你的日常工作。

## 技术栈

- **前端**: React 19 + TypeScript + Vite
- **桌面框架**: Tauri v2
- **UI**: Tailwind CSS v4 + Base UI
- **状态管理**: Zustand
- **路由**: React Router v7

## 开发

```bash
# 安装依赖
pnpm install

# 启动开发服务器
pnpm dev

# 启动 Tauri 开发环境
pnpm tauri dev

# 构建应用
pnpm tauri build
```

## 其他命令

```bash
pnpm lint          # 代码检查
pnpm lint:fix      # 自动修复 lint 问题
pnpm fmt           # 代码格式化
pnpm clear-cache   # 清除缓存
```

## License

MIT
