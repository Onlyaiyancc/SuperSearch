param([ValidateSet('dev','build','test','check','smoke')]$Mode = 'dev')
$ErrorActionPreference = 'Stop'
$projectPath = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectPath
# 只对当前进程设置工具链目录，不修改用户 PATH。
$env:CARGO_HOME = Join-Path $projectPath '.tools/cargo'
$env:RUSTUP_HOME = Join-Path $projectPath '.tools/rustup'
$env:PATH = (Join-Path $env:CARGO_HOME 'bin') + ';' + $env:PATH
$env:npm_config_cache = Join-Path $projectPath '.tools/npm-cache'
$env:TEMP = Join-Path $projectPath '.tools/tmp'
$env:TMP = $env:TEMP
New-Item -ItemType Directory -Force -Path $env:TEMP | Out-Null
if ($Mode -eq 'test' -or $Mode -eq 'check') {
    & cargo $Mode --manifest-path src-tauri/Cargo.toml
} elseif ($Mode -eq 'smoke') {
    & cargo run --manifest-path src-tauri/Cargo.toml -- --smoke-test
} else {
    & npm exec tauri $Mode
}
exit $LASTEXITCODE
