param([ValidateSet('dev','build','test','check','smoke')]$Mode = 'dev')
$ErrorActionPreference = 'Stop'
$projectPath = Split-Path -Parent $PSScriptRoot
# 与 npm 命令复用同一入口，避免两套工具链选择逻辑发生漂移。
& node (Join-Path $projectPath 'scripts/desktop.mjs') $Mode
exit $LASTEXITCODE
