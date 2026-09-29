# Meow 项目分析与架构基线

本文记录项目当前的真实结构和行为。它不是把现有实现全部判定为最佳实践，而是给后续修改提供共同上下文：哪些是稳定契约，哪些是已知技术债，哪些是新增功能应遵守的目标方向。

## 1. 项目定位

Meow 是一个基于 Tauri 2、Vue 3、Vite 和 Tailwind CSS 的本地漫画阅读器。当前核心场景是：

1. 用户选择一个本地目录。
2. Rust 读取目录项，前端筛选 `.zip` 文件。
3. Rust 使用 `zip` 读取压缩包，并在首页索引时将首张图片缩略后写入 SQLite 缓存。
4. 桌面端打开新的 Webview 窗口，移动端切换到 `/preview` 路由。
5. 预览页按自然排序读取 ZIP 中的条目并依次显示图片。

当前没有后端服务、远程数据源或全局状态管理库。`localStorage` 保存前端偏好，SQLite 仅作为应用缓存保存 ZIP 封面缩略图。

## 2. 运行时结构

```text
App.vue
├── TitleBar.vue                 全局标题栏、主题、桌面窗口控制
└── RouterView
    ├── IndexView.vue             目录选择、ZIP 列表、封面和打开阅读器
    └── PreviewView.vue           ZIP 条目读取和连续图片预览

Index/Preview
└── @tauri-apps/api/core
    └── src-tauri/src/lib.rs      Tauri command 入口
        └── src-tauri/src/utils
            ├── cover_cache.rs    ZIP 封面索引、缩略图和 SQLite 缓存
            ├── zip_reader.rs     ZIP 条目和文件读取
            ├── system.rs         目录读取、平台根路径、修改时间排序
            └── android_bridge.rs  Android JNI 状态栏桥接
```

### 主要模块

| 模块 | 当前职责 | 修改时的边界 |
| --- | --- | --- |
| `src/App.vue` | 应用外壳、标题栏和路由容器 | 不放页面业务和文件读取 |
| `src/components/TitleBar.vue` | 主题切换、桌面窗口拖动/最小化/最大化/关闭 | 原生权限和窗口 API 变化要同步 capability |
| `src/components/SelectPathDialog.vue` | 逐级浏览目录并返回所选路径 | 路径表示方式应由路径适配器统一 |
| `src/views/IndexView.vue` | 目录加载、ZIP 过滤、调用封面索引、预览窗口打开 | 页面只保留流程编排和搜索状态，不直接读取 ZIP |
| `src/views/PreviewView.vue` | 读取路由路径、展开 ZIP 图片、生成对象 URL | 后续应统一图片筛选、错误态和 URL 释放 |
| `src/utils/index.js` | 自然排序 | ZIP 和本地文件适配已下沉到 Rust，前端工具保持纯函数 |
| `src-tauri/src/lib.rs` | 注册 command 并转发到 Rust 实现 | 不继续堆积文件 I/O 和平台细节 |
| `src-tauri/src/utils/cover_cache.rs` | 以 ZIP 路径、修改时间和文件大小为指纹缓存封面缩略图 | 缓存文件属于应用缓存目录；不要让页面直接操作 SQLite |
| `src-tauri/src/utils/zip_reader.rs` | 读取 ZIP 条目和指定条目内容 | 保持 ZIP 错误包含文件路径和条目上下文 |
| `src-tauri/src/utils/system.rs` | 读取目录、Windows 驱动器、Android 根目录、修改时间排序 | 新增平台分支前先确认路径契约 |
| `src-tauri/src/utils/android_bridge.rs` | 获取 JVM、附加线程、调用 Android 状态栏方法 | 仅 Android 编译；JNI 失败必须可诊断 |

## 3. 当前跨层契约

### Tauri commands

| command | 输入 | 输出 | 当前调用方 | 备注 |
| --- | --- | --- | --- | --- |
| `read_dir_list` | `{ path: string }` | JSON 数组，项含 `name`、`is_dir`，排序实现会提供 `modified_time` | `IndexView`、`SelectPathDialog` | 空路径在 Windows/Android 有特殊根目录行为，Linux/macOS 当前会失败 |
| `read_zip_entries` | `{ path: string }` | `ZipEntry[]` | `PreviewView` | Rust 使用 `zip` 读取 ZIP 条目 |
| `read_zip_file` | `{ path: string, filename: string }` | `Vec<u8>` | `PreviewView` | Rust 使用 `zip` 读取指定条目 |
| `index_zip_covers` | `{ paths: string[] }` | `ZipCover[]`，含文件名、缩略图字节、MIME 和条目数量 | `IndexView` | SQLite 命中相同路径、修改时间和文件大小时直接返回缓存 |
| `read_file_data` | `{ path: string }` | 包含元数据和完整内容的 JSON | 当前未发现前端调用 | 候选清理项；大文件读取风险高，不要直接用于漫画阅读 |
| `update_theme_state` | `{ isDark: boolean }` | `Value` | `TitleBar` | Android 下进一步调用 JNI 修改状态栏；桌面端主要返回输入值 |

command 是 Rust 与前端之间的公共接口。新接口应优先使用明确的 `serde` 数据结构，而不是继续返回无类型的 `serde_json::Value`。

### 前端持久化

- `localStorage.target_path`：上次选择的目录。
- `localStorage.search_text`：首页最近一次搜索文本；用于首页组件被路由卸载或 Android Activity 重建后恢复筛选条件。
- `localStorage.theme`：主题值；不等于 `html.dark` 的实时 DOM 状态，切换时两者需要同时维护。

修改键名或数据格式时，要考虑已有用户的旧值和空值。读取本地值失败或路径失效时，界面必须能回到可重新选择目录的状态。

## 4. 文件读取与 ZIP 处理

首页封面索引由 Rust 的 `cover_cache` 模块负责：它读取 ZIP 条目、自然排序、提取首张图片，缩放为 JPEG thumbnail，并将结果存入应用缓存目录下的 SQLite 文件。缓存指纹是 ZIP 路径、文件修改时间和文件大小；任一指纹变化都会重新索引。

自然排序在 Rust 封面索引中通过 `natord::compare_ignore_case` 实现，前端阅读页通过 `filename.localeCompare(..., { numeric: true, sensitivity: 'base' })` 实现；两者都适合 `1, 2, 10` 这类漫画页名。任何改变排序的需求都应同时检查封面选择和阅读页顺序。

预览功能实际上有两套筛选规则：

- 封面提取只接受 `.jpg`、`.jpeg`、`.png`、`.gif`、`.bmp`、`.svg`、`.webp`。
- 阅读页目前只排除目录项，没有复用同一图片扩展名筛选。

后续修改应先定义“可阅读图片”的单一规则，再让封面、页数和阅读页复用它。对象 URL 的创建与释放也应集中管理，避免反复加载封面或页面后内存增长。

## 5. 平台与窗口行为

### 桌面端

- 主窗口使用无系统装饰的自定义标题栏。
- `TitleBar.vue` 负责拖动、最小化、最大化、关闭和主题切换。
- 打开漫画时创建新的 `WebviewWindow`，URL 为 `/preview?path=...`。
- 桌面窗口相关能力由 `src-tauri/capabilities/default.json` 授权。

### 移动端

- Android/iOS 通过 `@tauri-apps/plugin-os` 判断。
- 移动端不创建桌面子窗口，而是路由跳转到 `/preview`。
- `App.vue` 使用 `KeepAlive` 保留首页组件；返回首页时不重复加载目录，也不丢失当前筛选结果。搜索文本同时写入 `localStorage.search_text`，用于进程重建后的恢复。
- Android 的 `MainActivity.kt` 负责存储权限、透明状态栏和 JNI 初始化；Rust 通过 `android_bridge.rs` 调用状态栏文字颜色方法。

任何新窗口或原生能力都必须同时考虑这两条路径。不能只在桌面端验证后就认为移动端完成。

## 6. 已知问题与风险基线

以下是分析当前代码后确认的技术债。它们是后续排期或修复时的参考，不应在没有需求的情况下被顺手大规模重构。

### 已处理

- 首页搜索由 `zip_list` 保存完整列表、`render_list` 保存筛选结果；后续修改搜索时要保持“源列表不被筛选结果覆盖”的数据流。

### 高优先级行为风险

1. `saveTargetPath` 使用 `replaceAll("\\", "")` 去掉反斜杠，这不是合法的 Windows 路径规范化；再加上多处用字符串拼接路径，跨平台路径存在风险。
2. 桌面预览 URL 直接拼接路径，没有对 query 参数编码；路径含空格、`&`、`?` 或非 ASCII 字符时可能被错误解析。
3. 阅读页没有统一过滤非图片文件，封面显示的条目数量也包含目录和非图片条目，页面上的 `P` 语义可能不准确。
4. 创建的对象 URL 没有看到对应的 `URL.revokeObjectURL`；频繁加载封面或长漫画可能造成内存占用持续增加。
5. 大多数异步错误只写入控制台，用户看到的状态仍可能是空列表，无法区分“没有漫画”和“读取失败”。

### 中优先级结构问题

1. `IndexView.vue` 和 `PreviewView.vue` 都直接组合 `invoke`、文件大小、ZIP 解压和对象 URL，页面与基础设施耦合较高。
2. `read_path_list` 与 `read_path_list_by_sort` 存在重复实现；`greet`、`read_file_data` 当前也没有前端调用，后续应确认是否删除或补齐用途。
3. `system.rs` 用 `serde_json::Value` 拼目录项，字段契约不够稳定；排序按修改时间而不是名称，目录选择器的用户体验依赖未文档化的实现细节。
4. Vite 和 Tauri 开发配置硬编码了局域网 IP `192.168.166.103`，换机器或网络后开发启动可能失效。
5. Tauri capability 同时包含窗口通配符和较多权限；新增权限前应先缩小窗口范围并说明原因。
6. 当前没有完整的自动化测试套件、代码格式化脚本或 lint 脚本；`tests/index-view-state.test.mjs` 提供了首页筛选和状态恢复的最小纯函数回归测试。构建通过不代表路径、权限、ZIP 和移动端行为全部正确。

## 7. 后续功能的推荐改动路径

### 新增列表或目录能力

先在 Rust 定义稳定的目录项结构和排序语义，再由前端适配器转换为页面模型。页面只负责加载、筛选和展示，不在模板里拼接文件路径。

### 新增阅读能力

把“打开 ZIP、筛选图片、自然排序、生成/释放对象 URL、错误处理”集中到一个小接口，例如 `openArchive`/`listImages`/`readImage` 这一类服务。页面只消费图片项和加载状态。

### 新增原生能力

按以下顺序修改：

1. 先确定前端需要的最小接口和错误语义。
2. 在 Rust 实现 command 和内部模块。
3. 更新 capability 权限。
4. 在前端适配器中封装 `invoke`。
5. 最后接入页面并补桌面/移动端验证。

### 新增主题或视觉元素

先更新 `src/assets/style.css` 的主题变量，再使用 Tailwind 类引用。不要在单个组件中复制一套 light/dark 颜色值。

## 8. 验证基线

当前仓库可直接确认的命令：

```bash
npm run build
node --test tests/index-view-state.test.mjs
cargo fmt --check
cargo check
```

本次缓存改动中 `npm run build`、`cargo fmt --manifest-path src-tauri/Cargo.toml --check` 和允许网络后执行的 `cargo check --manifest-path src-tauri/Cargo.toml` 均成功；Vite 仍报告 Browserslist 数据过旧和 Tailwind 配置的 CommonJS/ESM 警告，但不是构建失败原因。

涉及 UI、窗口、路径、权限或 ZIP 的改动，还必须进行手动冒烟验证：

```text
启动 → 选择目录 → 显示 ZIP 封面 → 打开预览 → 检查顺序和图片 → 返回/关闭
```

涉及 Android 的改动，另行验证存储权限、状态栏主题、JNI 初始化和预览路由。
