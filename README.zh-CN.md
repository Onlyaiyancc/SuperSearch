# SuperSearch

[English](README.md) | **简体中文**

面向 Windows 的应用启动与文件名称搜索工具，基于 Tauri 2、Rust、Svelte 5 和 Everything SDK。通过键盘操作搜索栏，按名称、拼音、首字母和别名查找应用、文件及文件夹。

**[下载 v0.1.8](https://github.com/Onlyaiyancc/SuperSearch/releases/tag/v0.1.8)** · [全部版本](https://github.com/Onlyaiyancc/SuperSearch/releases) · [反馈问题](https://github.com/Onlyaiyancc/SuperSearch/issues/new?template=bug.yml)

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
| `SuperSearch_0.1.8_x64-setup.exe` | 推荐用于完整安装和专属索引服务，需要管理员权限。 |
| `SuperSearch-portable-x64.zip` | 完整解压后运行 `SuperSearch.exe`，保留全部许可文件；需事先安装 WebView2。 |
| `SuperSearch-full-x64.zip` | 推荐用于屏幕网址识别，随附 PDF/OCR 私有组件；Pandoc 与 Office 按需从官方来源一键下载安装。 |
| `SHA256SUMS.txt` | 发布附件的 SHA-256 校验值。 |
| `Release-Acceptance.md` | 对应构建的验证证据与剩余限制。 |

升级前先从系统托盘退出旧实例。安装包注册独立的 `Everything (SuperSearch)` 服务，避免与个人 Everything 安装混用。缺少 WebView2 时会联网下载运行时；下载失败后可恢复网络并重试。便携模式在专属服务不可用时使用目录扫描。首次索引可能需要几分钟；高级设置面板会自动刷新加载状态。

应用与安装包均未签名，Windows 可能显示未知发布者或 SmartScreen 提示。请从本仓库 Releases 页面下载，将文件哈希与 `SHA256SUMS.txt` 对照：

```powershell
Get-FileHash .\SuperSearch_0.1.8_x64-setup.exe -Algorithm SHA256
```

## 快速上手

| 快捷键 | 操作 |
| --- | --- |
| `Alt+Space` | 显示或隐藏搜索栏，可在设置中修改。 |
| `↑` / `↓` | 选择结果。 |
| `Space` | 启用文件预览后，用方向键选中文件再按空格预览；Esc 返回结果。 |
| `Enter` | 打开选中结果；本地搜索完成且无匹配时，使用必应搜索。 |
| `Ctrl+Enter` | 在文件资源管理器中定位选中的本地项目。 |
| `Shift+Enter` | 使用必应搜索当前输入。 |
| `Tab` | 循环选择可用的路径补全。 |
| `Ctrl+,` | 打开设置。 |
| `Esc` | 关闭当前面板或隐藏搜索栏。 |

在**设置 → 通用**中调整快捷键、主题和显示位置。显示位置可选“记住上次位置”或“跟随鼠标所在显示器”。通过结果操作菜单固定项目或编辑应用别名，在**设置 → 搜索**中配置搜索范围。

![设置界面：浏览器样例数据](docs/images/settings-preview.png)

## 隐私与验证范围

**屏幕扫码默认关闭。** 在**设置 → 扩展 → 屏幕扫码**中开启并保存，完整输入 `扫码` / `scan` 后回车，也支持 `qr`、`扫一扫`。搜索窗口会暂时隐藏，在本机识别鼠标所在显示器上直接显示的网址文字与二维码。网页链接排在二维码上方，每条可复制或打开，不会自动打开链接。首次使用会自动准备完整包中随附的文字识别组件，请保留 `components` 文件夹。可在扩展卡片中设置独立快捷键；不保存截图、OCR 全文或扫码历史。详细限制与验证范围见[屏幕扫码说明](docs/screen-qr.md)（英文）。

**格式转换默认关闭。** 在**设置 → 扩展**中开启并保存后，输入 `转换` / `convert`，或从文件操作菜单进入。支持常见图片、Office、Markdown、PDF 及扫描件文字识别；文档组件按需安装，在本机处理并保留原文件。格式范围、离线组件导入和转换效果限制见[使用说明](docs/format-conversion.md)（英文）。

**翻译与词典扩展默认关闭。** 在**设置 → 扩展**中开启并保存后，输入 `翻译 hello` / `translate hello` 英译中、`翻译 开发计划` / `translate 开发计划` 中译英，或 `查词 hello` / `dictionary hello` 查看英文音标、释义和例句；保留 `fy`、`dict` 简写。命令与内容用空格分隔，英文命令不区分大小写。按回车或点击提交按钮才联网，得到结果后再次回车可复制；翻译方向可手动调整。

扩展使用免密钥的 [MyMemory](https://mymemory.translated.net/doc/spec.php) 和 [Free Dictionary API](https://dictionaryapi.dev/)。只有主动提交的文本会发送给对应服务，不保存翻译历史。翻译每次最多 500 个 UTF-8 字节，查词支持单个英文单词；公共服务有额度与可用性限制。词典提供英文释义，中文含义请使用翻译。浏览器预览仅展示明确标注的合成样例，不连接这些服务或写入系统剪贴板。

普通运行默认不发送遥测。设置和使用历史保存在本机，可在设置中关闭或清除历史记录。主动触发网页搜索时，查询会交给默认浏览器和必应。

**文件快速预览和倒计时提醒也默认关闭**，可在**设置 → 扩展**中分别开启并保存。用方向键选中文件后按空格，或从结果菜单进入预览。仅读取常规本地文件，不支持网络位置和目录链接。

| 类型 | 大小与功能 |
| --- | --- |
| PDF | 100 MiB；Windows 本地渲染，支持前后翻页、页码跳转与缩放 |
| DOCX / XLSX / PPTX | 50 MiB；文字与表格内容，可切换工作表或幻灯片，不完整还原原排版 |
| PNG / JPEG / GIF / WebP / BMP / TIFF / ICO | 50 MiB，解码后最多 6400 万像素；大图使用有界缩略图，小图保持自适应高度 |
| 文本 / Markdown / 代码 / 配置 | 1 GiB；每段约 1 MiB，支持 UTF-8、UTF-16、GB18030 编码 |

HTML、SVG 仅显示源码，Markdown 不加载外部资源。Office 另有解压、XML 和输出限制，超过时拒绝预览或显示节选；最多预览 10 个工作表、每表 200 行与 40 列，或 100 张幻灯片。带外部关系、密码保护的文档及旧版 DOC/XLS/PPT 请使用关联应用打开。预览过程不会上传文件。

图片与 PDF 可在“适应窗口”和 10%–400% 缩放之间切换，放大后在内容区滚动查看。聚焦内容区后，`PageUp` / `PageDown` 切换 PDF 页或文本段；PDF 适应窗口时也可用左右方向键。`Ctrl` + `+` / `-` 缩放，`Ctrl+0` 恢复适应窗口。编辑页码或其他输入控件时保留原生按键行为；`Esc` 返回搜索。

输入 `提醒 20分钟 休息` / `remind 20m 休息`、`倒计时 30秒` / `timer 30s` 或 `提醒 1小时30分钟 开会`，按回车创建；输入 `提醒` / `remind` 查看和取消。也支持 `reminder`、`countdown`；命令与时长用空格分隔，英文命令不区分大小写。时间支持 s/秒、m/分钟、h/小时、d/天，大小写均可，也可输入 `1h30m` 或 `1d 2h` 等组合；支持 second/minute/hour/day 等完整写法。范围为 1 秒到 7 天，最多 20 个待执行提醒，记录保存在本机。隐藏窗口继续计时；退出程序期间无法即时通知，重新打开后恢复并合并提示已到期项。保存关闭此插件会取消所有待执行提醒。Windows 桌面通知显示 SuperSearch 应用名称和图标，请允许系统通知；系统关闭通知时可在提醒列表查看到期状态。浏览器预览只提供当前会话的合成样例，不发送系统通知。

本版确切构建、组件、OCR 与安装验收结果见[版本说明](docs/releases/v0.1.8.md)及附件 `Release-Acceptance.md`。v0.1.7 的云端安装记录仅作为旧版历史证据。

真实输入法、快捷键、休眠唤醒、断屏与 150%/200% 混合缩放、交互式 UAC 尚未由本次自动测试完整覆盖。云端 Windows 检查不覆盖全部 Windows 10/11 配置；旧版 30 分钟记录不替代本次发行程序的长期测试。

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
