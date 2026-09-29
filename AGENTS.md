# Meow 项目协作规范

这份文件是 Meow 项目的默认修改规则，适用于人工开发和代码代理。详细的现状分析、模块职责和已知技术债见 [`docs/architecture.md`](docs/architecture.md)。

## 先确认再修改

每次改动前完成以下检查：

1. 阅读 `docs/architecture.md` 中与本次功能相关的模块说明。
2. 确认改动属于哪一层：Vue 界面、前端适配器、Tauri 命令、Rust 文件系统实现，还是 Android 桥接。
3. 搜索现有实现和调用点，优先复用已有接口；不要为了一个小功能复制一套相似逻辑。
4. 如果改变了跨层数据结构、Tauri 命令、路径规则、窗口行为或平台行为，同步更新架构文档。

完成标准：修改范围、受影响的调用方和验证命令都能被明确说出来。

## 项目边界

Meow 是一个本地漫画 ZIP 阅读器：用户选择目录，应用扫描 `.zip` 文件，提取首张图片作为封面，并在阅读页按自然顺序读取压缩包内的图片。

稳定的调用方向如下：

```text
Vue View/Component
        ↓
前端适配器（Tauri invoke、文件路径、ZIP 读取、窗口打开）
        ↓
Tauri Command（src-tauri/src/lib.rs）
        ↓
Rust 实现（src-tauri/src/utils）
        ↓
操作系统文件系统 / Android JNI
```

依赖只能向下调用。页面可以调用适配器，适配器可以调用 Tauri 命令；Rust 不能反向依赖 Vue 状态或页面实现。

## 目录职责

- `src/views/`：路由页面和页面级流程。页面负责组合，不承载可复用的底层文件或 ZIP 逻辑。
- `src/components/`：可复用的 Vue 界面模块。组件通过 props、emits 或 slots 通信，不直接修改父级状态。
- `src/components/svg/`：纯展示型 SVG 组件，不在其中加入业务逻辑。
- `src/utils/`：前端底层适配和纯函数。涉及 Tauri、ZIP、路径的逻辑应集中在这里或后续拆出的 `src/services/`，不要散落在多个页面。
- `src/router/`：路由表和导航配置；窗口打开策略不在路由表里实现。
- `src/assets/style.css`：全局 Tailwind 入口和主题 CSS 变量。新增颜色、主题值先在这里定义，再在组件中引用。
- `src-tauri/src/lib.rs`：Tauri 命令的薄入口和应用装配。命令参数校验、调用实现和错误转换要保持简短。
- `src-tauri/src/utils/`：Rust 文件、目录、平台和 Android JNI 实现。纯数据转换尽量与文件 I/O 分开。
- `src-tauri/capabilities/default.json`：Tauri 权限清单。新增窗口、文件或系统能力时必须同时检查权限。
- `src-tauri/tauri.conf.json`、`vite.config.js`：构建、开发服务器和窗口配置。不要在业务页面中硬编码这些配置。
- `src-tauri/gen/android/`：当前仓库中受版本控制的 Android 工程。修改前先确认是否为 Tauri 生成内容；`MainActivity.kt` 含有自定义 JNI、状态栏和存储权限逻辑，不能在重新生成时无意识覆盖。
- `dist/`、`src-tauri/target/`、`src-tauri/gen/schemas/`：构建或生成产物，不作为业务代码修改。

## 新功能放在哪里

按下面的规则选择修改点：

- 新增页面：在 `src/views/` 添加页面，在 `src/router/` 注册路由。
- 新增可复用交互：在 `src/components/` 添加组件；用明确的 props/emits 定义接口。
- 新增 ZIP、文件读取、路径处理：放入前端适配器/服务，不在模板事件处理器里直接实现。
- 新增原生能力：在 Rust 添加 Tauri command 和实现，前端通过一个集中适配器调用；同时更新 `capabilities/default.json`。
- 新增 Android 专属能力：用 `#[cfg(target_os = "android")]` 隔离 Rust 代码，并同步维护 Kotlin/JNI 入口和权限说明。
- 只有当两个实现确实需要替换时才引入新的 seam/adapter；不要为单一实现制造空壳抽象。抽象的目标是让复杂行为隐藏在小接口后面。

## Vue 规则

- 使用 Vue 3 `<script setup>`；组件内顺序保持为：imports → props/emits → state → computed → lifecycle → handlers → template → style。
- JavaScript 变量和函数使用 `camelCase`；Rust/Tauri 命令边界保留 `snake_case`，新命令由前端适配器用 camelCase 方法包装。
- 一个状态只有一个拥有者。父组件状态通过 props 下传，子组件通过 emits 请求修改；不要同时在多个页面维护同一份可变副本。
- 页面中只保留页面流程和组合逻辑。重复的异步加载、错误处理、路径拼接和资源释放要抽到适配器或服务。
- 所有异步用户操作都要有成功、失败和加载状态；`console.error` 不能作为唯一的用户反馈。
- 使用 `URL.createObjectURL` 后必须在不再使用时 `URL.revokeObjectURL`，尤其是重新加载列表、离开预览页和组件卸载时。
- 路径不是 URL。文件路径拼接使用统一的路径工具；放入路由 query 或 Webview URL 前必须进行 URL 编码，不能直接字符串拼接。
- 事件处理器使用命名函数；模板中避免用分号串联多个副作用。
- 新增样式优先使用现有 Tailwind 工具类和全局主题变量；不要在组件里重复定义颜色值。空的 `<style scoped>` 不保留。

## Tauri 与 Rust 规则

- Tauri command 是跨层接口：命名、参数、返回值和错误信息一旦被前端使用，就按公共契约维护。
- 新 command 使用明确的 Rust 类型或 `serde` 结构体；不要继续扩大无类型 `serde_json::Value` 的使用范围。
- command 只做参数校验、调用领域/基础设施实现和错误转换；文件遍历、分块读取、平台分支放在 `src-tauri/src/utils/`。
- 外部路径和文件内容都视为不可信输入。使用 `Path`/`PathBuf` 处理路径，校验文件类型和边界，避免用字符串 `+` 拼接路径。
- 面向用户的失败返回 `Result<T, String>` 或可序列化错误结构，并带有操作上下文；不要对文件、JNI 或用户输入使用会导致进程崩溃的 `unwrap`/`expect`。
- 每个新增 command 都要检查 `capabilities/default.json`，只授予所需的最小权限；窗口列表和通配符权限变更需要在说明中写明原因。
- 平台专属代码必须用条件编译隔离。桌面、Android、iOS 的行为差异写在对应适配器旁边，不在页面里堆平台判断。
- Rust 改动后至少运行 `cargo fmt --check` 和 `cargo check`；涉及纯函数或命令契约时补测试。

## 数据与状态契约

- `target_path` 是用户选择的目录，当前持久化键为 `localStorage.target_path`。
- `theme` 当前持久化键为 `localStorage.theme`，值为 `dark` 或 `light`；主题 DOM 状态由 `html.dark` 控制。
- 目录项至少包含 `name` 和 `is_dir`；`modified_time` 是当前排序实现使用的可选字段。
- ZIP 条目排序使用自然排序；封面和阅读页必须对“目录项、非图片文件、图片文件”的筛选规则保持一致。
- 修改上述契约时，必须同时检查 `IndexView.vue`、`PreviewView.vue`、`SelectPathDialog.vue`、`src-tauri/src/lib.rs` 和 `src-tauri/src/utils/system.rs`。

## 验证规则

当前项目还没有完整的前端测试脚本或 Rust 测试套件；已有的纯函数回归测试可直接用 Node 执行。因此每次改动至少执行与范围匹配的验证：

- 前端或配置改动：`npm run build`
- 涉及首页筛选/状态恢复：`node --test tests/index-view-state.test.mjs`
- Rust 改动：`cargo fmt --check`、`cargo check`
- 路由、窗口、文件路径或 ZIP 行为改动：在桌面端手动验证“选择目录 → 显示 ZIP → 打开预览 → 返回/关闭窗口”。
- Android 专属改动：额外验证存储权限、状态栏主题和预览页；不能用桌面端通过代替 Android 验证。

验证完成标准：命令结果记录清楚；如果因网络、平台工具链或设备不可用而未验证，必须明确写出未验证项和原因。

## 修改纪律

- 一次改动围绕一个功能或一个缺陷，避免顺手重命名、格式化整棵目录或升级无关依赖。
- 先保持现有行为，再做结构改进；重构必须能说明它降低了重复、风险或跨层耦合。
- 不要把现有技术债复制到新代码中。已知问题见 `docs/architecture.md`，新增代码应遵守目标规则。
- 修改完成后检查 `git diff`，确认没有构建产物、个人机器 IP、密钥或无关文件进入提交。

## 完成检查清单

提交或交付前应满足：

- 改动位于正确的模块和 seam，调用方向没有反转。
- 新增或修改的跨层契约、权限、平台行为已同步文档。
- 异步失败、资源释放和路径编码已处理。
- 与改动范围匹配的构建、检查或手动验证已完成并记录。
- `git diff` 只包含本次需求的文件和内容。
