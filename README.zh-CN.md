# SuperSearch

[English](README.md) | **简体中文**

面向 Windows 的应用启动与文件名称搜索工具，基于 Tauri 2、Rust、Svelte 5 和 Everything SDK。通过键盘操作搜索栏，按名称、拼音、首字母和别名查找应用、文件及文件夹。

**[下载 v0.1.7](https://github.com/Onlyaiyancc/SuperSearch/releases/tag/v0.1.7)** · [全部版本](https://github.com/Onlyaiyancc/SuperSearch/releases) · [反馈问题](https://github.com/Onlyaiyancc/SuperSearch/issues/new?template=bug.yml)

仅支持 Windows x64，应用界面目前为简体中文。发行程序**未签名**，运行需要 Microsoft WebView2 Runtime。

![搜索界面：合成样例数据](docs/images/search-preview.png)

## 功能

- 按名称搜索已安装的应用、文件和文件夹；应用匹配支持拼音、首字母和别名。
- 使用可配置的全局快捷键呼出搜索栏，通过键盘选择结果。
- 成功打开过的匹配项优先，支持固定项目和自定义搜索范围。
- 使用专属 Everything 服务快速索引 NTFS；服务不可用或文件系统不支持时回退到目录扫描。
- 支持浅色、深色和跟随系统主题；窗口可记住上次位置，也可在鼠标所在显示器居中呼出。
- 支持主动网页搜索；本地搜索完成且无匹配结果时，可回退到必应搜索。

## 下载与安装

从[最新正式版](https://github.com/Onlyaiyancc/SuperSearch/releases/latest)选择附件：

| 附件 | 用途 |
| --- | --- |
| `SuperSearch_0.1.7_x64-setup.exe` | 推荐用于完整安装和专属索引服务，需要管理员权限。 |
| `SuperSearch-portable-x64.zip` | 完整解压后运行 `SuperSearch.exe`，保留全部许可文件；需事先安装 WebView2。 |
| `SHA256SUMS.txt` | 发布附件的 SHA-256 校验值。 |
| `Release-Acceptance.md` | 对应构建的验证证据与剩余限制。 |

升级前先从系统托盘退出旧实例。安装包注册独立的 `Everything (SuperSearch)` 服务，避免与个人 Everything 安装混用。缺少 WebView2 时会联网下载运行时；下载失败后可恢复网络并重试。便携模式在专属服务不可用时使用目录扫描。首次索引可能需要几分钟；高级设置面板会自动刷新加载状态。

应用与安装包均未签名，Windows 可能显示未知发布者或 SmartScreen 提示。请从本仓库 Releases 页面下载，将文件哈希与 `SHA256SUMS.txt` 对照：

```powershell
Get-FileHash .\SuperSearch_0.1.7_x64-setup.exe -Algorithm SHA256
```

## 快速上手

| 快捷键 | 操作 |
| --- | --- |
| `Alt+Space` | 显示或隐藏搜索栏，可在设置中修改。 |
| `↑` / `↓` | 选择结果。 |
| `Enter` | 打开选中结果；本地搜索完成且无匹配时，使用必应搜索。 |
| `Ctrl+Enter` | 在文件资源管理器中定位选中的本地项目。 |
| `Shift+Enter` | 使用必应搜索当前输入。 |
| `Tab` | 循环选择可用的路径补全。 |
| `Ctrl+,` | 打开设置。 |
| `Esc` | 关闭当前面板或隐藏搜索栏。 |

在**设置 → 通用**中调整快捷键、主题和显示位置。显示位置可选“记住上次位置”或“跟随鼠标所在显示器”。通过结果操作菜单固定项目或编辑应用别名，在**设置 → 搜索**中配置搜索范围。

![设置界面：浏览器样例数据](docs/images/settings-preview.png)

## 隐私与验证范围

普通运行默认不发送遥测。设置和使用历史保存在本机，可在设置中关闭或清除历史记录。主动触发网页搜索时，查询会交给默认浏览器和必应。

v0.1.7 的确切安装包已在临时 Windows 运行器上通过升级、数据保留、服务查询、卸载共存和全新安装检查。缺少 WebView2 的测试在 Windows Server 2025 上验证了下载失败后的清理及恢复网络后的安装。确切便携程序通过了短时原生界面与资源诊断。

维护者已反馈本地候选版的输入法、快捷键与单实例、数据保留、引擎重启、睡眠唤醒、断屏定位及交互式 UAC 正常；这些实际交互未独立核对运行文件哈希。真实 150%/200% 缩放及依赖错误弹窗措辞仍未单独验证；Server 2025 检查不覆盖所有 Windows 10/11 配置。旧版 30 分钟记录不替代本次发行程序的长期测试。具体范围见[版本说明](docs/releases/v0.1.7.md)及随版本提供的验收报告。

截图和浏览器预览使用合成数据，不执行原生搜索或启动本地应用。

## 从源码构建

准备 Windows x64、Git、Node.js 24 LTS、Rust stable MSVC、包含 Windows SDK 的 Visual Studio C++ Build Tools，以及 WebView2。安装与排查步骤见[开发指南](docs/development.md)（英文）。

```powershell
git clone https://github.com/Onlyaiyancc/SuperSearch.git
cd SuperSearch
npm ci
npm run desktop:dev
```

验证并构建：

```powershell
npm run verify -- --smoke
npm run notices -- --check
npm run desktop:build
```

Rust 命令统一通过 `npm run desktop:*` 运行。启动器优先使用已有项目本地工具链，缺少时使用标准 Rust 安装，无需复制维护者的 `.tools/`。`npm run dev` 启动使用样例数据的浏览器预览。打包与发布流程见[发布指南](docs/releasing.md)（英文）。

## 项目结构

| 目录 | 内容 |
| --- | --- |
| [src](src/) | Svelte / TypeScript 前端。 |
| [src-tauri](src-tauri/) | Rust 后端、安装器、内置引擎和原始许可。 |
| [public](public/) | 静态资源与图标源文件。 |
| [scripts](scripts/) | 构建、诊断、许可生成与发布工具。 |
| [tests](tests/) | 前端与工具链测试。 |
| [docs](docs/README.md) | 开发、发布、设计及验证文档。 |

依赖、构建缓存、个人配置及本地诊断资料不提交到 Git。本地交付放在 `artifacts/releases/`，历史资料放在 `artifacts/archive/`。

## 参与与许可

欢迎反馈可复现的问题并提交聚焦的改进，参见 [CONTRIBUTING.md](CONTRIBUTING.md)；漏洞按 [SECURITY.md](SECURITY.md) 私下报告。

SuperSearch 原创代码使用 [MIT 许可证](LICENSE)。内置 Everything、SDK 及其它依赖保留各自条款，本项目许可证不替代其授权。详见[第三方声明](THIRD-PARTY-NOTICES.md)及[依赖许可全文](docs/licenses/THIRD-PARTY-LICENSES.txt)。SuperSearch 独立于 voidtools，不代表其背书。
