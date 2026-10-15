# SuperSearch

专注应用启动与文件/文件夹名称查找的 Windows 桌面搜索器。当前版本 **0.1.6，本地测试版，未签名**。

界面为银白中性灰搜索栏，默认 680×76，中文输入与反馈。结果区高度为 172＋76×行数，最多六行；设置窗口高 620。呼出清空，有结果才展开，使用过的匹配项优先。无匹配 Enter 用 Bing 搜索，Shift＋Enter 强制网页搜索；同一次运行中保留拖动后的窗口位置。

Everything 原厂签名后台程序和许可证已内置。机器安装包通过一次管理员授权安装 SuperSearch 专属服务；普通用户客户端自动使用固定 NTFS 磁盘的 MFT/USN 高速索引。其它文件系统和服务缺失时回退目录扫描，不改变已有 Everything。界面采用银白、炭灰与黑白应用图标。客户端异常退出或专属服务停止时，下次查询/状态检查会尝试恢复；每 30 秒最多两次自动恢复，设置中的手动重启可重置限制。仍需系统 WebView2。

## 使用与交付

本轮交付目录为 `outputs/Priority-Optimization-2026-10-15/`，运行其中的 `SuperSearch_0.1.6_x64-setup.exe` 或 `SuperSearch.exe`。发布完成后，`outputs/Latest-Release.json` 指向最新经过校验的交付。使用便携程序前先从托盘退出已经运行的旧程序，再启动新构建。默认托盘待命，Alt＋Space 呼出；鼠标悬停左侧放大镜时显示齿轮，点击进入设置，也可按 Ctrl＋,。键盘聚焦该入口时同样显示齿轮。搜索栏右侧不显示“设置”文字，启动快捷键可在悬停提示和设置页查看。

说明、技术文档、设计对照、验收记录、引擎来源/许可证和校验值位于 outputs/。浏览器预览只有标注样例，拒绝系统与后台引擎动作。

## 开发

Tauri2 / Rust / Svelte5 / Everything SDK / SQLite，依赖由 package-lock.json 与 Cargo.lock 固定。项目工具链入口只为子进程设置本地环境，不改全局 PATH。

```powershell
npm ci
npm test
npm run check
npm run desktop:test
npm run desktop:check
npm run desktop:dev
npm run desktop:build
npm run verify -- --smoke
npm run desktop:audit:install
npm run verify -- --smoke --audit-rust
npm run desktop:soak -- -DurationSeconds 1800
npm run release -- --with-diagnostics
```

内置引擎诊断：发布程序 `SuperSearch.exe --smoke-test-bundled`，或开发目录 `node work/run-bundled-smoke.mjs`。只生成并索引独立测试目录，不依赖共享 Everything 服务，不执行用户文件动作。原 `--smoke-test` 仅保留为外部客户端的兼容诊断。

测试套件包括 139 项前端/工具链测试、107 项 Rust 测试，另有 1 项外部客户端 opt-in 探针默认忽略。统一入口 `npm run verify -- --smoke --audit-rust` 依次运行测试、静态检查、前端构建、Rust 检查、生产 npm 依赖审计、隔离索引诊断和 Rust 审计，首次失败立即停止并写入 `test-results/verification.json`。索引诊断包括自有客户端异常退出恢复和缓存索引重启。以本轮报告中的实际运行结果为准。

原生 UI 与资源采样：先构建发布程序，再运行 `powershell -NoProfile -File scripts/benchmark.ps1`；报告写入 `test-results/ui-native-benchmark.json` 与 `test-results/resource-usage.json`。默认索引 483 个独立夹具；加 `-FixtureFiles 100000 -TimeoutSeconds 900` 可运行十万文件验收。首屏与最终结果分别计时，资源采样包含诊断进程的 WebView2 和 Everything 客户端。

长期诊断：`npm run desktop:soak -- -DurationSeconds 1800`，反复隐藏、显示、搜索及收起窗口，并记录可见/隐藏空闲。原生诊断报告与按进程角色采样分别写入 `test-results/ui-native-soak.json`、`resource-soak.json` 和原始 `resource-soak-samples.ndjson`。长诊断窗口不接受人工交互或抢占焦点；CPU 按逻辑处理器总数归一化，空闲统计剔除跨阶段的采样区间，内存增长斜率不能单独证明泄漏。完整安装生命周期、真实输入法/多屏、睡眠唤醒和高速服务停止/恢复仍需适合的原生环境单独验收。

本轮还增加固定容量的应用拼音衍生键缓存、修正“调音”的读音、支持服务恢复后按需从目录模式切回高速索引，并把呼出时可能等待后台 IPC 的状态检查移出 UI 线程。原有成功使用历史优先规则保留。交付会验证版本、构建时效、完整检查结果、EXE 哈希以及原生/资源报告和原始采样之间的哈希对应；失败时不更新 latest 指针。

## 第三方软件

SDK 来源与原始许可在 src-tauri/vendor/；内置 unmodified signed Everything.exe 和完整声明在 src-tauri/resources/everything/，来源/版本/哈希已记录。后台仍为独立进程，SDK 只是 IPC 桥接，不是独立索引引擎库。

高速诊断：`SuperSearch.exe --smoke-test-fast`，需要专属服务运行；报告验证创建/改名/删除和目录过滤。视觉验收见 `design-qa.md`。历史启用记录 `outputs/Fast-Activation-Report.json` 不代表当前机器的服务状态，应以程序诊断或本次测试报告为准。
