# 只由 native-installation.ps1 在一次性托管运行器调用；加载函数本身没有系统副作用。
function Invoke-WebView2Acceptance {
    if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows') { throw 'Disposable Windows GitHub-hosted runner required.' }
    if ($env:GITHUB_REPOSITORY -ne 'Onlyaiyancc/SuperSearch' -or $env:GITHUB_WORKSPACE -ne $taskRoot) { throw 'Unexpected acceptance workspace.' }
    if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Administrative runner required.' }
    if ((Test-Path -LiteralPath $taskExe) -or (Get-Service $taskService -ErrorAction SilentlyContinue)) { throw 'Candidate must be uninstalled before dependency acceptance.' }
    $taskWebView = [ordered]@{ outcome='failed'; environment='Disposable GitHub-hosted Windows, elevated silent installer; no UAC or dialog wording validation'; candidateSha=$CandidateSha; installerSha256=(Get-FileHash -LiteralPath $taskSetup -Algorithm SHA256).Hash; checks=[ordered]@{} }
    $taskRuntimeRoot = Join-Path ${env:ProgramFiles(x86)} 'Microsoft/EdgeWebView/Application'
    $taskUserRuntimeRoot = Join-Path $env:LOCALAPPDATA 'Microsoft/EdgeWebView/Application'
    $taskRegistry = @('HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'HKCU:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}')
    function Get-RuntimeState {
        [ordered]@{
            versions=@($taskRegistry | ForEach-Object { (Get-ItemProperty -LiteralPath $_ -Name pv -ErrorAction SilentlyContinue).pv } | Where-Object { $_ -and $_ -ne '0.0.0.0' })
            executables=@(Get-ChildItem -Path (Join-Path $taskRuntimeRoot '*/msedgewebview2.exe'),(Join-Path $taskUserRuntimeRoot '*/msedgewebview2.exe') -File -ErrorAction SilentlyContinue | Select-Object -ExpandProperty FullName)
        }
    }
    function Assert-NoRuntime {
        $taskState = Get-RuntimeState
        if ($taskState.versions.Count -or $taskState.executables.Count) { throw 'WebView2 runtime is still present; missing-runtime evidence is invalid.' }
    }
    try {
        $taskReport.phase='webview2-remove-runtime'
        $taskWebView.before=Get-RuntimeState
        if (Get-Process msedgewebview2 -ErrorAction SilentlyContinue) { throw 'WebView2 is still in use on the disposable runner.' }
        # 仅运行固定系统目录中微软签名的卸载器，不删除主机文件或伪造运行时检测键。
        $taskRuntimeSetup = @(Get-ChildItem -Path (Join-Path $taskRuntimeRoot '*/Installer/setup.exe') -File -ErrorAction SilentlyContinue | Sort-Object { [version]$_.Directory.Parent.Name } -Descending | Select-Object -First 1)
        if ($taskRuntimeSetup.Count) {
            $taskSignature = Get-AuthenticodeSignature -LiteralPath $taskRuntimeSetup[0].FullName
            if ($taskSignature.Status -ne 'Valid' -or $taskSignature.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation') { throw 'Runtime uninstaller is not verified Microsoft code.' }
            $taskRemoval = Start-Process -FilePath $taskRuntimeSetup[0].FullName -ArgumentList @('--uninstall','--msedgewebview','--system-level','--force-uninstall','--verbose-logging') -PassThru -WindowStyle Hidden
            if (-not $taskRemoval.WaitForExit(120000)) { & taskkill.exe /PID $taskRemoval.Id /T /F | Out-Null; throw 'Runtime uninstall exceeded its deadline.' }
            $taskRemoval.Refresh()
            $taskWebView.runtimeUninstallExitCode=$taskRemoval.ExitCode
        }
        Wait-Condition { $taskState=Get-RuntimeState; -not $taskState.versions.Count -and -not $taskState.executables.Count } 'Cannot prepare a genuinely missing WebView2 runtime.' 30
        Assert-NoRuntime
        $taskWebView.afterRemoval=Get-RuntimeState
        $taskWebView.checks.runtimeAbsent='passed'
        $taskReport.phase='webview2-download-blocked'
        $taskRuleName='SuperSearch-Acceptance-Blocked-Download-'+[Guid]::NewGuid().ToString('N')
        if (@(Get-NetFirewallProfile | Where-Object { -not $_.Enabled }).Count) { throw 'Firewall profiles must already be enabled for a valid download-block test.' }
        try {
            # 只阻断本次安装器出站连接，运行器控制连接及其他程序不受影响。
            New-NetFirewallRule -Name $taskRuleName -DisplayName $taskRuleName -Direction Outbound -Program $taskSetup -Action Block -Enabled True -Profile Any | Out-Null
            Assert-NoRuntime
            $taskBlocked = Start-Process -FilePath $taskSetup -ArgumentList '/S' -PassThru -WindowStyle Hidden
            if (-not $taskBlocked.WaitForExit(180000)) { & taskkill.exe /PID $taskBlocked.Id /T /F | Out-Null; throw 'Blocked installer did not fail within its deadline.' }
            $taskBlocked.Refresh()
            $taskWebView.blockedInstallerExitCode=$taskBlocked.ExitCode
            if ($taskBlocked.ExitCode -eq 0) { throw 'Blocked dependency download incorrectly reported success.' }
            Assert-NoRuntime
            if ((Test-Path -LiteralPath $taskExe) -or (Get-Service $taskService -ErrorAction SilentlyContinue)) { throw 'Failed dependency install left a partial program or service.' }
            $taskWebView.checks.downloadFailureAndNoPartialInstall='passed'
        } finally { Remove-NetFirewallRule -Name $taskRuleName -ErrorAction SilentlyContinue }
        $taskReport.phase='webview2-online-retry'
        Assert-NoRuntime
        Invoke-Checked $taskSetup @('/S') 300
        Assert-DedicatedService
        Assert-InstalledPayload
        Test-InstalledApp
        $taskWebView.afterInstall=Get-RuntimeState
        if (-not $taskWebView.afterInstall.versions.Count -or -not $taskWebView.afterInstall.executables.Count) { throw 'Installer did not restore the WebView2 runtime.' }
        $taskWebView.checks.onlineRetryAndRuntimeInstall='passed'
        # 真实 WebView 前端必须完成诊断，不能只凭进程存活判定界面可用。
        $taskUiRoot=Join-Path $taskOutput 'webview2-ui-workspace'
        New-Item -ItemType Directory -Path $taskUiRoot -Force | Out-Null
        Push-Location $taskUiRoot
        try { Invoke-Checked $taskExe @('--ui-benchmark') 180 }
        finally { Pop-Location }
        $taskUiFile=Join-Path $taskUiRoot 'test-results/ui-native-benchmark.json'
        Copy-Item -LiteralPath $taskUiFile -Destination (Join-Path $taskOutput 'webview2-ui.json')
        $taskUi=Get-Content -LiteralPath $taskUiFile -Raw | ConvertFrom-Json
        if ($taskUi.outcome -ne 'passed' -or -not $taskUi.native -or -not $taskUi.visibleWindow) { throw 'Restored runtime did not render the native application.' }
        $taskWebView.checks.realWebViewRenderAndQueries='passed'
        Uninstall-Candidate
        $taskWebView.checks.candidateCleanup='passed'
        $taskWebView.outcome='passed'
    } catch { $taskWebView.error=$_.Exception.Message; throw }
    finally { $taskWebView.timestamp=[DateTime]::UtcNow.ToString('o'); $taskWebView | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $taskOutput 'webview2.json') -Encoding utf8 }
}
