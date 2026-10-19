# Contributing

感谢帮助改进 SuperSearch。界面使用简体中文，项目代码注释优先使用中文，开发脚本输出使用英文。

## 开始

按 [开发说明](docs/development.md) 准备 Windows x64 环境。提交问题前搜索已有 Issues，说明使用版本、系统版本、安装或便携模式、复现步骤、预期和实际结果。截图和报告请先脱敏。

较大的功能变更请先通过 Issue 讨论需求与范围。小修复可以直接提交 PR；保持改动聚焦，并为实际修复的行为补回归验证。贡献的原创代码按本项目 MIT 许可证提供，第三方内容需保留其原始授权。

## 验证

```powershell
npm test
npm run check
npm run desktop:test
npm run desktop:check
```

涉及引擎时追加 `npm run desktop:smoke:bundled`；涉及 UI 时检查浅色、深色及窄屏。依赖变化后运行 `npm run notices` 并提交更新。不要提交 `.tools/`、`node_modules/`、构建产物、测试数据、证书或密钥。

PR 应描述触发问题、最终行为、验证结果及尚未覆盖的边界。安全问题按 [SECURITY.md](SECURITY.md) 私下报告。
