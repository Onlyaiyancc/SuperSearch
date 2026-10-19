# 开发与构建

## Windows 首次准备

目标平台是 Windows x64，原生后端依赖 Windows API 和 Everything SDK，不能直接在 Linux/macOS 上运行测试。

1. 安装 Git 和 [Node.js](https://nodejs.org/) 24 LTS。前端最低要求 Node.js 22.12；本地验证版本为 Node.js 24.11.1。
2. 安装 [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)，勾选 **使用 C++ 的桌面开发**、MSVC x64 编译工具与 Windows SDK。
3. 从 [rustup.rs](https://rustup.rs/) 安装 Rust，选择 MSVC 工具链，安装后重新打开终端。`rust-toolchain.toml` 选择 stable / x86_64-pc-windows-msvc；本地验证的 Rust 版本为 1.99.0。
4. 安装或确认已有 [Microsoft Edge WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)。

官方前置要求见 [Tauri Windows prerequisites](https://v2.tauri.app/start/prerequisites/#windows)。

```powershell
git clone https://github.com/Onlyaiyancc/SuperSearch.git
cd SuperSearch
npm ci
npm test
npm run check
npm run desktop:test
npm run desktop:check
npm run desktop:smoke:bundled
npm run desktop:dev
```

不需要复制维护者的 `.tools/`。`scripts/desktop.mjs` 优先使用现有项目本地 Rust；缺少它时使用标准安装的 Rust，保留其 `CARGO_HOME` / `RUSTUP_HOME`。命令只调整子进程环境，不修改全局 PATH。所有 Cargo 操作通过 `npm run desktop:*` 入口调用。

## 构建与检查

```powershell
npm run notices
npm run verify -- --smoke
npm run desktop:build
```

安装包在 `src-tauri/target/release/bundle/nsis/`，便携程序在 `src-tauri/target/release/supersearch.exe`。请先从托盘退出正在使用同一路径的旧程序，以免 Windows 阻止替换 EXE。

Rust 审计需先运行 `npm run desktop:audit:install`，然后运行 `npm run verify -- --smoke --audit-rust`。首次构建会下载锁文件对应的依赖和打包工具，需要网络连接。锁文件应提交到仓库。

## 调试与隐私

`npm run dev` 提供浏览器预览：仅使用带有“预览样例”标记的合成数据，不访问本机搜索或启动应用。原生测试夹具和报告分别位于 `.tools/`、`test-results/`，不提交到 Git。

普通运行默认不发送遥测；成功打开项目的路径和使用次数可保存在本机，可在设置中关闭或清除。主动使用网页搜索时，查询会交给默认浏览器和必应。导出诊断前请检查并移除个人路径与设备信息。

真实输入法、多屏、安装/升级/卸载、睡眠唤醒需要单独的原生验收。短时基准和合成查询不能证明长期资源行为。

## 依赖更新

Dependabot 每周检查依赖。npm/Cargo 只自动合并**补丁更新到同一个 PR**，不会自动合并 PR；0.x 的次版本可能不兼容，需要单独评估。GitHub Actions 将次版本和补丁更新分组，所有 Action 仍固定到完整提交 SHA。版本更新 PR 的并发上限分别为 3、3、2；安全更新保持启用，不受这些版本更新上限影响。

在依赖分支上运行 `npm ci`，然后运行 `npm run notices`，检查并提交锁文件和 `docs/licenses/THIRD-PARTY-LICENSES.txt` 的变化。CI 的 `Dependency notices are stale` 表示依赖与清单不一致；不要删除许可证检查，也不要手工只替换版本号。出现 `Missing upstream license texts` 时，核对对应版本的上游许可证后补充 `docs/licenses/upstream/sources.json` 及原文。

提交前运行 `npm audit`、`npm run notices -- --check` 和 `npm run verify -- --smoke --audit-rust`。Tauri/原生依赖更新还需构建发行程序并运行隔离烟测。升级 Action 时，CI 会上传并重新下载构建产物，核对文件名和 SHA-256；该检查不会发布新版本。

不要用 `--force` 或 `--legacy-peer-deps` 掩盖前端版本冲突。Vite 与 Svelte Vite 插件应配套评估，TypeScript 的版本必须满足 svelte-check 的 peer 范围。Beta 的人工验收见 [原生验收清单](testing/beta-native-acceptance.md)。
