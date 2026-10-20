# SuperSearch

面向 Windows 的应用启动与文件名称搜索工具，基于 Tauri 2、Rust、Svelte 5 和 Everything SDK。简体中文界面，支持拼音匹配、快捷键呼出、文件搜索与本地使用历史排序。

当前源码 **0.1.7，正式版候选，尚未公开发布，未签名**。已公开的测试版为 [v0.1.6-beta.2](https://github.com/Onlyaiyancc/SuperSearch/releases/tag/v0.1.6-beta.2)。仅支持 Windows x64，需要 WebView2 Runtime。

![搜索界面：合成样例数据](docs/images/search-preview.png)

## 下载与使用

公开安装包和便携 ZIP 位于 [GitHub Releases](https://github.com/Onlyaiyancc/SuperSearch/releases)。候选版本的验收完成前，请使用已发布的 beta 或按[开发文档](docs/development.md)从源码构建。下载后可用随版本提供的 SHA256SUMS.txt 校验文件；未签名程序可能触发 Windows SmartScreen 提示。

- 默认 **Alt＋Space** 呼出，**Ctrl＋,** 打开设置。
- 输入名称或拼音查找应用、文件及文件夹，方向键选择，Enter 打开，Tab 补全。
- 成功打开过的匹配项优先；无本地匹配时 Enter 使用必应搜索，Shift＋Enter 强制网页搜索。
- 支持别名、固定项目、搜索范围，以及浅色、深色和跟随系统主题。
- 可在通用设置中选择“记住上次位置”或“跟随鼠标所在显示器”；鼠标跟随在呼出时居中，展开结果时保持当前位置。

安装包通过管理员授权安装独立的 `Everything (SuperSearch)` 服务，为 NTFS 固定磁盘提供高速索引，避免与个人 Everything 安装混用。服务不可用或其它文件系统使用目录扫描。首次建立索引需要时间；设置页会在打开索引面板时持续刷新加载状态。

升级前先从托盘退出旧实例。便携 ZIP 需保留随附许可文件；完整安装体验和服务注册使用安装包。

## 数据与限制

普通运行默认不发送遥测。使用历史和设置保存在本机，可在设置中关闭历史记录或清除数据。主动触发网页搜索时，查询会交给默认浏览器和必应。

浏览器预览和本页截图使用合成样例，不执行本地搜索或启动操作。beta.2 的输入法、双屏、安装生命周期和 30 分钟诊断证据见其发布附件；这些记录不替代 0.1.7 候选验收。屏幕热插拔、睡眠唤醒、真实 150%/200% 缩放及缺少 WebView2 的安装仍需验证。短时测试不能证明长期资源表现。

![设置界面：浏览器样例](docs/images/settings-preview.png)

## 开发

准备 Windows x64、Node.js 24 LTS、Rust stable MSVC、Visual Studio C++ Build Tools 和 WebView2，然后运行：

```powershell
npm ci
npm run desktop:dev
```

检查与构建：

```powershell
npm run verify -- --smoke
npm run notices -- --check
npm run desktop:build
```

Rust 命令统一使用 `npm run desktop:*`。启动器优先使用已有项目本地工具链，缺少时使用标准安装的 Rust，不需要复制维护者的 `.tools/`。完整步骤见[开发与构建](docs/development.md)，发布流程见[发布指南](docs/releasing.md)。

| 目录 | 内容 |
| --- | --- |
| [src](src/) | Svelte / TypeScript 前端 |
| [src-tauri](src-tauri/) | Rust 后端、安装器、内置引擎与原始许可 |
| [public](public/) | 静态资源与图标源文件 |
| [scripts](scripts/) | 构建、诊断、许可生成与发布工具 |
| [tests](tests/) | 前端和工具链测试 |
| [docs](docs/README.md) | 开发、设计、验证范围与发布说明 |

依赖、构建缓存、个人工具配置、诊断和本地交付归档不提交到 Git。根目录只保留常见项目文件与必要目录；本地交付放在 `artifacts/releases/`，历史材料放在 `artifacts/archive/`。

## 参与和许可

欢迎提交可复现的问题和改进，参见 [CONTRIBUTING.md](CONTRIBUTING.md)。漏洞报告方式见 [SECURITY.md](SECURITY.md)。

SuperSearch 原创代码使用 [MIT 许可证](LICENSE)。内置的未修改 Everything 程序、Everything SDK 和其它依赖保留各自许可；MIT 不覆盖这些组件。详见[第三方声明](THIRD-PARTY-NOTICES.md)及[依赖许可全文](docs/licenses/THIRD-PARTY-LICENSES.txt)。本项目不隶属于 voidtools，也不代表其背书。
