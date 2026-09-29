# Meow

Meow 是一个使用 Tauri 2 + Vue 3 + Vite 构建的本地漫画 ZIP 阅读器。它支持目录浏览、ZIP 封面预览和连续图片阅读；桌面端使用独立预览窗口，移动端使用 `/preview` 路由。

## 开发

```bash
npm install
npm run dev
```

生产构建：

```bash
npm run build
```

Rust/Tauri 代码位于 `src-tauri/`。具体的开发约束、模块职责、跨平台注意事项和验证标准见 [`AGENTS.md`](AGENTS.md) 与 [`docs/architecture.md`](docs/architecture.md)。

## 推荐 IDE

- VS Code
- Volar
- Tauri
- rust-analyzer
