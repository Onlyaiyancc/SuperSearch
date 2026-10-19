#Requires -Version 7
param(
    [Parameter(Mandatory)][string]$CandidateRun,
    [Parameter(Mandatory)][string]$CandidateSha
)
$ErrorActionPreference = 'Stop'
# 本脚本会安装/卸载程序和测试服务，绝不能在日常系统上运行。
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') { throw 'Disposable GitHub-hosted runner required.' }
if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Administrative runner required.' }
if ($CandidateRun -notmatch '^\d+$' -or $CandidateSha -notmatch '^[a-f0-9]{40}$') { throw 'Invalid candidate identity.' }
$taskRepo = 'Onlyaiyancc/SuperSearch'
$taskRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location -LiteralPath $taskRoot
$taskInstall = Join-Path $env:ProgramFiles 'SuperSearch'
$taskExe = Join-Path $taskInstall 'supersearch.exe'
$taskData = Join-Path $env:LOCALAPPDATA 'com.supersearch.desktop'
$taskDefaultService = 'Everything'
$taskService = 'Everything (SuperSearch)'
if ((Test-Path -LiteralPath $taskInstall) -or (Test-Path -LiteralPath $taskData) -or (Get-Service $taskService,$taskDefaultService -ErrorAction SilentlyContinue)) { throw 'Runner already contains a relevant installation; refusing to modify it.' }
$taskOutput = Join-Path $taskRoot 'test-results/native-installation'
New-Item -ItemType Directory -Force -Path $taskOutput | Out-Null
$taskReport = [ordered]@{ outcome='failed'; candidateRun=$CandidateRun; candidateSha=$CandidateSha; baselineTag='v0.1.6-beta.1'; checks=[ordered]@{}; environment='Disposable GitHub-hosted Windows runner, silent elevated installation; UAC interaction not exercised'; phase='download' }
$taskReport.os=(Get-CimInstance Win32_OperatingSystem | Select-Object Caption,Version,BuildNumber)
function Invoke-Checked([string]$File, [string[]]$Arguments, [int]$Timeout = 180) {
    $taskChild = Start-Process -FilePath $File -ArgumentList $Arguments -PassThru -WindowStyle Hidden
    if (-not $taskChild.WaitForExit($Timeout * 1000)) { & taskkill.exe /PID $taskChild.Id /T /F | Out-Null; throw "Process exceeded deadline: $([IO.Path]::GetFileName($File))" }
    $taskChild.Refresh()
    if ($taskChild.ExitCode -ne 0) { throw "Process failed: $([IO.Path]::GetFileName($File)), exit=$($taskChild.ExitCode)" }
}
function Wait-Condition([scriptblock]$Condition, [string]$Message, [int]$Seconds = 30) {
    $taskDeadline = [DateTime]::UtcNow.AddSeconds($Seconds)
    do { if (& $Condition) { return }; Start-Sleep -Milliseconds 500 } while ([DateTime]::UtcNow -lt $taskDeadline)
    throw $Message
}
function Assert-DedicatedService {
    Wait-Condition { (Get-Service $taskService -ErrorAction SilentlyContinue).Status -eq 'Running' } 'Dedicated service not running.'
    $taskImage = (Get-ItemProperty -LiteralPath "HKLM:\SYSTEM\CurrentControlSet\Services\$taskService").ImagePath
    if (-not $taskImage.StartsWith('"' + (Join-Path $taskInstall 'engine/Everything.exe') + '"', [StringComparison]::OrdinalIgnoreCase)) { throw 'Unexpected dedicated service executable.' }
    if (-not $taskImage.Contains('SuperSearch Everything Service')) { throw 'Dedicated service pipe missing.' }
}
function Test-InstalledApp {
    $taskChild = Start-Process -FilePath $taskExe -ArgumentList '--show' -PassThru -WindowStyle Hidden
    try {
        Wait-Condition { (Test-Path -LiteralPath (Join-Path $taskData 'config.json')) -and (Test-Path -LiteralPath (Join-Path $taskData 'state.db')) } 'Installed application did not initialize its store.'
        Start-Sleep -Seconds 3
        $taskChild.Refresh()
        if ($taskChild.HasExited) { throw 'Installed application exited unexpectedly.' }
    } finally {
        $taskChild.Refresh()
        if (-not $taskChild.HasExited) { & taskkill.exe /PID $taskChild.Id /T /F | Out-Null; if ($LASTEXITCODE -ne 0) { throw 'Cannot stop diagnostic application tree.' } }
    }
}
function Test-FastIndex([string]$Name) {
    Invoke-Checked $taskExe @('--smoke-test-fast') 180
    $taskSmoke = Get-Content 'test-results/fast-native-smoke.json' -Raw | ConvertFrom-Json
    if ($taskSmoke.outcome -ne 'passed' -or $taskSmoke.engine.indexing -ne 'ntfs-usn') { throw 'Installed NTFS/USN diagnostic failed.' }
    Copy-Item -LiteralPath 'test-results/fast-native-smoke.json' -Destination (Join-Path $taskOutput "$Name-fast.json")
}
function Assert-InstalledPayload {
    $taskActual=(Get-FileHash -LiteralPath $taskExe -Algorithm SHA256).Hash
    $taskReport.installedExecutableSha256=$taskActual
    if ($taskActual -ne $taskExpectedExe) { throw 'Installed executable differs from the verified installer payload.' }
    foreach($taskPair in @(@('licenses/LICENSE.txt','LICENSE'),@('licenses/THIRD-PARTY-NOTICES.md','THIRD-PARTY-NOTICES.md'),@('licenses/THIRD-PARTY-LICENSES.txt','THIRD-PARTY-LICENSES.txt'),@('engine/License.txt','Everything-License.txt'))) {
        if ((Get-FileHash -LiteralPath (Join-Path $taskInstall $taskPair[0]) -Algorithm SHA256).Hash -ne (Get-FileHash -LiteralPath (Join-Path $taskOutput ('portable/'+$taskPair[1])) -Algorithm SHA256).Hash) { throw 'Installed license resource differs from candidate notices.' }
    }
}
function Uninstall-Candidate {
    $taskUninstaller = Join-Path $taskInstall 'uninstall.exe'
    if (-not (Test-Path -LiteralPath $taskUninstaller)) { throw 'Uninstaller missing.' }
    Invoke-Checked $taskUninstaller @('/S')
    Wait-Condition { -not (Get-Service $taskService -ErrorAction SilentlyContinue) -and -not (Test-Path -LiteralPath $taskExe) } 'Uninstall did not remove program and dedicated service.'
}
try {
    $taskRun = gh api "repos/$taskRepo/actions/runs/$CandidateRun" | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0 -or $taskRun.head_sha -ne $CandidateSha -or $taskRun.head_branch -ne 'main' -or $taskRun.event -ne 'push' -or $taskRun.conclusion -ne 'success' -or $taskRun.workflow_id -ne 388914064) { throw 'Candidate is not a successful main Windows build.' }
    gh run download $CandidateRun --repo $taskRepo --name windows-release --dir (Join-Path $taskOutput 'candidate')
    if ($LASTEXITCODE -ne 0) { throw 'Candidate download failed.' }
    $taskCandidate = Join-Path $taskOutput 'candidate'
    $taskNames = @()
    foreach ($taskLine in Get-Content (Join-Path $taskCandidate 'SHA256SUMS.txt')) {
        if ($taskLine -notmatch '^([a-f0-9]{64})  (SuperSearch_0\.1\.6_x64-setup\.exe|SuperSearch-portable-x64\.zip)$') { throw 'Unexpected candidate checksum entry.' }
        $taskExpected=$Matches[1]; $taskName=$Matches[2]; $taskNames += $taskName
        if ((Get-FileHash -LiteralPath (Join-Path $taskCandidate $taskName) -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskExpected) { throw 'Candidate hash mismatch.' }
    }
    if ($taskNames.Count -ne 2 -or @($taskNames | Select-Object -Unique).Count -ne 2) { throw 'Candidate manifest incomplete.' }
    gh release download v0.1.6-beta.1 --repo $taskRepo --pattern 'SuperSearch_0.1.6_x64-setup.exe' --dir (Join-Path $taskOutput 'baseline')
    if ($LASTEXITCODE -ne 0) { throw 'Baseline download failed.' }
    $taskBaseline = Join-Path $taskOutput 'baseline/SuperSearch_0.1.6_x64-setup.exe'
    if ((Get-FileHash -LiteralPath $taskBaseline -Algorithm SHA256).Hash.ToLowerInvariant() -ne '7f6f65e0c05423c27808cfbc7d57ceb86931fac32031fe365ea4a24a0e6bb965') { throw 'Baseline hash mismatch.' }
    Expand-Archive -LiteralPath (Join-Path $taskCandidate 'SuperSearch-portable-x64.zip') -DestinationPath (Join-Path $taskOutput 'portable')
    $taskSetup = Join-Path $taskCandidate 'SuperSearch_0.1.6_x64-setup.exe'
    # NSIS 打包可修改 bundle 类型标记；应和安装包自身的载荷比对。
    $taskSevenZip=(Get-Command 7z.exe -ErrorAction Stop).Source
    & $taskSevenZip x -y ('-o'+(Join-Path $taskOutput 'installer-payload')) $taskSetup | Out-Null
    if($LASTEXITCODE -ne 0){throw 'Cannot inspect candidate installer payload.'}
    $taskPayload=@(Get-ChildItem -LiteralPath (Join-Path $taskOutput 'installer-payload') -Recurse -File -Filter supersearch.exe)
    if($taskPayload.Count -ne 1){throw 'Expected one executable in candidate installer.'}
    $taskExpectedExe=(Get-FileHash -LiteralPath $taskPayload[0].FullName -Algorithm SHA256).Hash
    $taskReport.installerSha256=(Get-FileHash -LiteralPath $taskSetup -Algorithm SHA256).Hash
    $taskReport.executableSha256=$taskExpectedExe
    $taskReport.portableExecutableSha256=(Get-FileHash -LiteralPath (Join-Path $taskOutput 'portable/SuperSearch.exe') -Algorithm SHA256).Hash
    $taskReport.phase='baseline-install'
    Invoke-Checked $taskBaseline @('/S')
    Assert-DedicatedService
    Test-InstalledApp
    $taskReport.baselineInstalledExecutableSha256=(Get-FileHash -LiteralPath $taskExe -Algorithm SHA256).Hash
    & node scripts/native-installation-store.mjs seed
    if ($LASTEXITCODE -ne 0) { throw 'Cannot seed synthetic upgrade state.' }
    $taskReport.checks.baselineInstall='passed'
    $taskReport.phase='upgrade'
    Invoke-Checked $taskSetup @('/S')
    Assert-DedicatedService
    Assert-InstalledPayload
    Test-InstalledApp
    & node scripts/native-installation-store.mjs check
    if ($LASTEXITCODE -ne 0) { throw 'Upgrade did not preserve synthetic state.' }
    Test-FastIndex 'upgrade'
    $taskReport.checks.upgradeAndStateRetention='passed'
    $taskReport.phase='coexistence'
    $taskPersonal = Join-Path $taskOutput 'personal-Everything.exe'
    Copy-Item -LiteralPath (Join-Path $taskInstall 'engine/Everything.exe') -Destination $taskPersonal
    Invoke-Checked $taskPersonal @('-install-service')
    Wait-Condition { (Get-Service $taskDefaultService -ErrorAction SilentlyContinue).Status -eq 'Running' } 'Control Everything service did not start.'
    $taskPersonalImage=(Get-ItemProperty -LiteralPath "HKLM:\SYSTEM\CurrentControlSet\Services\$taskDefaultService").ImagePath
    Uninstall-Candidate
    if ((Get-Service $taskDefaultService).Status -ne 'Running' -or (Get-ItemProperty -LiteralPath "HKLM:\SYSTEM\CurrentControlSet\Services\$taskDefaultService").ImagePath -ne $taskPersonalImage) { throw 'Uninstall modified the control Everything service.' }
    & node scripts/native-installation-store.mjs check
    if ($LASTEXITCODE -ne 0) { throw 'Uninstall unexpectedly removed retained settings.' }
    $taskReport.checks.uninstallAndEverythingCoexistence='passed'
    $taskReport.phase='fresh-candidate-install'
    # 已核实为本次生成的固定测试目录；移走后验证全新用户数据初始化。
    $taskExpectedData=[IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'com.supersearch.desktop'))
    if ([IO.Path]::GetFullPath($taskData) -ne $taskExpectedData) { throw 'Unexpected fixture directory.' }
    Move-Item -LiteralPath $taskData -Destination (Join-Path $taskOutput 'retained-synthetic-store')
    Invoke-Checked $taskSetup @('/S')
    Assert-DedicatedService
    Assert-InstalledPayload
    Test-InstalledApp
    Test-FastIndex 'fresh'
    $taskLink = Join-Path $env:ProgramData 'Microsoft/Windows/Start Menu/Programs/SuperSearch.lnk'
    if (-not (Test-Path -LiteralPath $taskLink)) { throw 'Start Menu shortcut missing.' }
    $taskReport.checks.freshCandidateInstall='passed'
    $taskReport.phase='final-uninstall'
    Uninstall-Candidate
    if ((Get-Service $taskDefaultService).Status -ne 'Running') { throw 'Control service changed during second uninstall.' }
    $taskReport.checks.finalUninstall='passed'
    $taskReport.outcome='passed'
} catch { $taskReport.error=$_.Exception.Message; throw }
finally {
    $taskReport.timestamp=[DateTime]::UtcNow.ToString('o')
    $taskReport | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $taskOutput 'report.json') -Encoding utf8
}
