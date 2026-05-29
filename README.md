# CodexBox

基于 [CodexPlusPlus](https://github.com/BigPizzaV3/CodexPlusPlus) 优化而来的 Codex App 本地增强启动器。通过本地代理聚合多供应商模型，用户选择模型即可自动路由到对应上游。

## 核心功能

- **统一模型聚合**：所有供应商的模型汇总成一张列表，选择模型即自动路由，无需手动切换供应商
- **协议转换引擎**：Responses ↔ Chat Completions 双向转换，兼容不同供应商的 API 协议
- **CDP 桥接注入**：通过 Chromium DevTools Protocol 注入增强脚本
- **智能路由引擎**：6 种路由策略、健康检查、故障转移、模型映射
- **用户脚本管理**：脚本市场 + 自定义脚本
- **会话管理**：删除、导出 Markdown、撤销、跨工作区移动

## 技术栈

| 层 | 选型 |
|----|------|
| 后端 | Rust 2024 + tokio |
| 前端 | Tauri 2.x + React 19 + TypeScript + Vite 6 + Tailwind 4 |
| UI | shadcn/ui |
| 存储 | SQLite (rusqlite) |
| HTTP | reqwest |

## 快速开始

### 开发模式
```bash
cd apps/codex-plus-manager
npm install
npm run dev
```

### 构建 DMG（macOS）
```bash
cd apps/codex-plus-manager
npm run build
```

## 许可证

MIT License — 详见 [LICENSE](LICENSE)
