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
