param(
    [string]$Executable = 'src-tauri/target/release/SuperSearch.exe',
    [ValidateRange(10,1200)][int]$TimeoutSeconds = 180,
    [ValidateRange(483,1000000)][int]$FixtureFiles = 483
)
$ErrorActionPreference = 'Stop'
$taskProject = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $taskProject
$taskExecutable = (Resolve-Path -LiteralPath $Executable).Path
$taskStartedAt = [DateTime]::UtcNow
# 仅启动并采样本次隔离诊断进程树，不控制日常程序或系统服务。
$taskProcess = Start-Process -FilePath $taskExecutable -ArgumentList @('--ui-benchmark','--benchmark-files',"$FixtureFiles") -WorkingDirectory $taskProject -WindowStyle Hidden -PassThru
$taskSamples = [System.Collections.Generic.List[object]]::new()
$taskPreviousCpu = @{}
$taskClock = [Diagnostics.Stopwatch]::StartNew()
$taskPreviousMs = 0.0
$taskWebViewObserved = $false
try {
    while (-not $taskProcess.HasExited) {
        if ($taskClock.Elapsed.TotalSeconds -ge $TimeoutSeconds) { throw 'Native benchmark exceeded its deadline.' }
        $taskRows = @(Get-CimInstance Win32_Process -Filter "Name = 'supersearch.exe' OR Name = 'msedgewebview2.exe' OR Name = 'Everything.exe'")
        $taskIds = [System.Collections.Generic.HashSet[uint32]]::new()
        [void]$taskIds.Add([uint32]$taskProcess.Id)
        do {
            $taskAdded = $false
            foreach ($taskRow in $taskRows) {
                if ($taskIds.Contains([uint32]$taskRow.ParentProcessId) -and $taskIds.Add([uint32]$taskRow.ProcessId)) { $taskAdded = $true }
            }
        } while ($taskAdded)
        $taskPrivate = 0L
        $taskWorking = 0L
        $taskCpuDelta = 0.0
        $taskCount = 0
        foreach ($taskRow in $taskRows) {
            if (-not $taskIds.Contains([uint32]$taskRow.ProcessId)) { continue }
            try {
                $taskChild = Get-Process -Id $taskRow.ProcessId -ErrorAction Stop
                $taskStartTime = $taskChild.StartTime
                $taskCpuTime = $taskChild.TotalProcessorTime
                # 短寿命子进程可能已退出，避免空时间属性中断整次诊断。
                if ($null -eq $taskStartTime -or $null -eq $taskCpuTime) { continue }
                $taskIdentity = "$($taskChild.Id)|$($taskStartTime.ToUniversalTime().Ticks)"
                $taskCpu = $taskCpuTime.TotalMilliseconds
                if ($taskPreviousCpu.ContainsKey($taskIdentity)) { $taskCpuDelta += [Math]::Max(0, $taskCpu - $taskPreviousCpu[$taskIdentity]) }
                $taskPreviousCpu[$taskIdentity] = $taskCpu
                $taskPrivate += $taskChild.PrivateMemorySize64
                $taskWorking += $taskChild.WorkingSet64
                $taskCount++
                if ($taskRow.Name -eq 'msedgewebview2.exe') { $taskWebViewObserved = $true }
            } catch [Microsoft.PowerShell.Commands.ProcessCommandException] {
                # 两次读取之间进程可能正常退出；下一个采样继续。
            }
        }
        $taskNowMs = $taskClock.Elapsed.TotalMilliseconds
        if ($taskCount -gt 0) {
            $taskCpuPercent = if ($taskPreviousMs -eq 0) { $null } else { [Math]::Round(100 * $taskCpuDelta / ($taskNowMs - $taskPreviousMs) / [Environment]::ProcessorCount, 3) }
            $taskSamples.Add([pscustomobject][ordered]@{ elapsedMs = [Math]::Round($taskNowMs); processCount = $taskCount; privateMiB = [Math]::Round($taskPrivate / 1MB, 2); workingSetMiB = [Math]::Round($taskWorking / 1MB, 2); cpuPercent = $taskCpuPercent })
        }
        $taskPreviousMs = $taskNowMs
        Start-Sleep -Milliseconds 500
        $taskProcess.Refresh()
    }
    $taskReportPath = Join-Path $taskProject 'test-results/ui-native-benchmark.json'
    if (-not (Test-Path -LiteralPath $taskReportPath) -or (Get-Item -LiteralPath $taskReportPath).LastWriteTimeUtc -lt $taskStartedAt) { throw 'Native benchmark did not produce a fresh report.' }
    $taskUiReport = Get-Content -LiteralPath $taskReportPath -Raw | ConvertFrom-Json
    $taskSummary = [ordered]@{
        outcome = if ($taskUiReport.outcome -eq 'passed' -and $taskUiReport.visibleWindow -and $taskWebViewObserved) { 'passed' } else { 'failed' }
        executableSha256 = (Get-FileHash -LiteralPath $taskExecutable -Algorithm SHA256).Hash
        nativeReportSha256 = (Get-FileHash -LiteralPath $taskReportPath -Algorithm SHA256).Hash
        measurement = 'Short isolated UI benchmark process tree, including WebView2 and owned Everything client. Excludes machine service; CPU is normalized by logical processor count. Not an idle or long-duration resource test.'
        sampleCount = $taskSamples.Count
        webViewObserved = $taskWebViewObserved
        peakPrivateMiB = ($taskSamples | Measure-Object -Property privateMiB -Maximum).Maximum
        peakWorkingSetMiB = ($taskSamples | Measure-Object -Property workingSetMiB -Maximum).Maximum
        samples = $taskSamples.ToArray()
    }
    $taskSummary | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath test-results/resource-usage.json -Encoding utf8
    Write-Output "Native UI benchmark: $($taskSummary.outcome); samples=$($taskSummary.sampleCount); peakPrivateMiB=$($taskSummary.peakPrivateMiB)"
    if ($taskSummary.outcome -ne 'passed') { throw "Native UI benchmark failed: $($taskUiReport.error); ownedWebViewObserved=$taskWebViewObserved" }
} finally {
    $taskProcess.Refresh()
    if (-not $taskProcess.HasExited) { Stop-Process -Id $taskProcess.Id }
}
