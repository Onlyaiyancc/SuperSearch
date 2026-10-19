# 发布流程

GitHub 下载入口是 [Releases](https://github.com/Onlyaiyancc/SuperSearch/releases)。`artifacts/` 与测试输出属于维护者本地资料，不提交到源码仓库。

1. 在同一个提交中更新 `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` 的版本，并更新锁文件及 `docs/releases/<标签>.md`。beta 标签格式为 `vX.Y.Z-beta.N`，正式版为 `vX.Y.Z`，必须与源码版本完全一致。
2. 依赖变化后运行 `npm run notices`。按 [Beta 原生验收清单](testing/beta-native-acceptance.md) 记录候选构建的验收结果与未验证项。确认原生 EXE、安装包和第三方许可完整，私钥/证书/诊断数据不在提交中。
3. 推送经过审查的源码提交到 main，等待 **Windows checks and pre-release** 完成。该流程包含完整 Git 历史扫描、锁文件安装、许可校验、前后端测试、静态检查、Windows 打包和隔离引擎诊断。
4. 完成候选验收并获得相应发布授权后，从 Actions 手动运行同一工作流，在 tag 中填写标签。仅手动填写标签时，流程才有发布写权限；beta 标签发布 Pre-release，正式标签只创建草稿。不得将创建正式版草稿视为公开发布授权。
5. 发布资产包含安装包、携带许可文件的便携 ZIP 和 SHA-256 校验值。发布后重新下载核对校验值，并确认标签对应的源码提交。

当前公开测试版为 [`v0.1.6-beta.2`](https://github.com/Onlyaiyancc/SuperSearch/releases/tag/v0.1.6-beta.2)，源码正在准备 0.1.7。程序未签名，Release 说明必须明确标注实际候选/正式状态、管理员安装服务、WebView2 需求及未完整验收的原生边界。签名密钥只存于安全的签名服务或 GitHub 加密 Secret，不能进入 Git 历史。

## 0.1.7 正式版准备

- 保持 tag 为空运行构建；记录成功构建的完整 SHA 与 run ID，再手动启用 `native_acceptance`，显式填写这两个值。验收会读取该 SHA 的 package.json，验证比固定的 0.1.6 基线更新，再核对安装包/便携包/安装后 PE 版本和原始文件哈希。
- `--smoke-test-fast` 的 `initialIndex` 记录引擎首次观测、provider 首次就绪、夹具首次命中、查询错误与最后状态；引擎 `startupMs` 仅是提取/启动耗时，不是全盘索引耗时。首次观测时间受轮询和 IPC 延迟影响。保留失败报告，不能用重试通过覆盖冷启动超时记录。
- 为同一候选重新运行安装升级、资源和原生交互验收。签名状态、热插拔、睡眠唤醒、150%/200% DPI、UAC 与缺少 WebView2 场景逐项列出；上一 beta 的通过结果只能作为历史参考。
- 本轮仅准备候选，不创建公开标签或 Release。用户验收后决定是否创建正式草稿；草稿资产重新下载校验后，再经用户明确授权公开。

仓库所有者还应启用私密漏洞报告、Dependabot alerts 与可用的 secret scanning/push protection，再将通过的 CI 检查设为主分支合并要求。这些设置可能需要额外的仓库管理权限。

本地严格留档流程仍可使用 `npm run release -- --output artifacts/releases/<新目录> --with-diagnostics`；它要求额外的长时间与规模化诊断证据，不应使用旧报告替代新构建的验证。
