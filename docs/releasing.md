# 发布流程

GitHub 下载入口是 [Releases](https://github.com/Onlyaiyancc/SuperSearch/releases)。`artifacts/` 与测试输出属于维护者本地资料，不提交到源码仓库。

1. 在同一个提交中更新 `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` 的版本，并更新锁文件及 `docs/releases/<标签>.md`。标签格式为 `vX.Y.Z-beta.N`，基础版本必须一致。
2. 依赖变化后运行 `npm run notices`。确认原生 EXE、安装包和第三方许可完整，私钥/证书/诊断数据不在提交中。
3. 推送经过审查的源码提交到 main，等待 **Windows checks and pre-release** 完成。该流程包含完整 Git 历史扫描、锁文件安装、许可校验、前后端测试、静态检查、Windows 打包和隔离引擎诊断。
4. 从 Actions 手动运行同一工作流，在 tag 中填写对应 beta 标签。仅手动填写标签时，流程才有发布写权限；它从已验证的提交生成标签并发布 Pre-release。
5. 发布资产包含安装包、携带许可文件的便携 ZIP 和 SHA-256 校验值。发布后重新下载核对校验值，并确认标签对应的源码提交。

首次公开版本拟为 `v0.1.6-beta.1`。当前程序未签名，Release 说明必须明确标注 beta 状态、管理员安装服务、WebView2 需求及未完整验收的原生边界。签名密钥只存于安全的签名服务或 GitHub 加密 Secret，不能进入 Git 历史。

仓库所有者还应启用私密漏洞报告、Dependabot alerts 与可用的 secret scanning/push protection，再将通过的 CI 检查设为主分支合并要求。这些设置可能需要额外的仓库管理权限。

本地严格留档流程仍可使用 `npm run release -- --output artifacts/releases/<新目录> --with-diagnostics`；它要求额外的长时间与规模化诊断证据，不应使用旧报告替代新构建的验证。
