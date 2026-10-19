param(
    [string]$Executable = 'src-tauri/target/release/SuperSearch.exe',
    [ValidateRange(1,3600)][int]$DurationSeconds = 1800,
    [ValidateRange(483,1000000)][int]$FixtureFiles = 483,
    [ValidateRange(500,10000)][int]$SampleIntervalMs = 2000
)
$ErrorActionPreference = 'Stop'
$taskProject = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $taskProject
$taskExecutable = (Resolve-Path -LiteralPath $Executable).Path
$taskStartedAt = [DateTime]::UtcNow
$taskHash = (Get-FileHash -LiteralPath $taskExecutable -Algorithm SHA256).Hash
$taskReportRoot = Join-Path $taskProject 'test-results'
New-Item -ItemType Directory -Force -Path $taskReportRoot | Out-Null
$taskSamplesPath = Join-Path $taskReportRoot 'resource-soak-samples.ndjson'
$taskMetadataPath = Join-Path $taskReportRoot 'resource-soak-metadata.json'
$taskNativePath = Join-Path $taskReportRoot 'ui-native-soak.json'
$taskWriter = [IO.StreamWriter]::new($taskSamplesPath, $false, [Text.UTF8Encoding]::new($false))
$taskPreviousCpu = @{}
$taskClock = [Diagnostics.Stopwatch]::StartNew()
$taskPreviousUnixMs = $null
$taskProcess = $null
$taskSampleCount = 0
$taskServices = @(Get-CimInstance Win32_Service -Filter "Name = 'Everything (SuperSearch)'")
$taskServiceId = if ($taskServices.Count -gt 0) { [uint32]$taskServices[0].ProcessId } else { 0 }
$taskServiceState = if ($taskServices.Count -gt 0) { $taskServices[0].State } else { 'NotInstalled' }
try {
    # 只有本次隔离诊断进程会被控制；专属服务仅作为独立资源参考读取。
    $taskProcess = Start-Process -FilePath $taskExecutable -ArgumentList @('--ui-soak','--soak-seconds',"$DurationSeconds",'--benchmark-files',"$FixtureFiles") -WorkingDirectory $taskProject -WindowStyle Hidden -PassThru
    while (-not $taskProcess.HasExited) {
        $taskSetupBudget = if ($FixtureFiles -gt 10000) { 900 } else { 240 }
        if ($taskClock.Elapsed.TotalSeconds -gt ($DurationSeconds + $taskSetupBudget)) { throw 'Native soak exceeded its deadline.' }
        $taskRows = @(Get-CimInstance Win32_Process -Filter "Name = 'supersearch.exe' OR Name = 'msedgewebview2.exe' OR Name = 'Everything.exe'")
        $taskIds = [System.Collections.Generic.HashSet[uint32]]::new()
        [void]$taskIds.Add([uint32]$taskProcess.Id)
        do {
            $taskAdded = $false
            foreach ($taskRow in $taskRows) {
                if ($taskIds.Contains([uint32]$taskRow.ParentProcessId) -and $taskIds.Add([uint32]$taskRow.ProcessId)) { $taskAdded = $true }
            }
        } while ($taskAdded)
        $taskNowUnixMs = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
        $taskRoles = [ordered]@{}
        $taskLiveIdentities = [System.Collections.Generic.HashSet[string]]::new()
        foreach ($taskRole in @('app','webview','engine','service')) {
            $taskRoles[$taskRole] = [ordered]@{ processCount = 0; privateMiB = 0.0; workingSetMiB = 0.0; cpuPercent = if ($null -eq $taskPreviousUnixMs) { $null } else { 0.0 } }
        }
        foreach ($taskRow in $taskRows) {
            $taskPid = [uint32]$taskRow.ProcessId
            if (-not $taskIds.Contains($taskPid) -and $taskPid -ne $taskServiceId) { continue }
            $taskRole = if ($taskPid -eq $taskServiceId) { 'service' } elseif ($taskPid -eq $taskProcess.Id) { 'app' } elseif ($taskRow.Name -eq 'msedgewebview2.exe') { 'webview' } else { 'engine' }
            try {
                $taskChild = Get-Process -Id $taskPid -ErrorAction Stop
                $taskStartTime = $taskChild.StartTime
                $taskCpuTime = $taskChild.TotalProcessorTime
                # 进程可能在枚举后退出，或其时间属性不可读取；跳过该次不完整采样。
                if ($null -eq $taskStartTime -or $null -eq $taskCpuTime) { continue }
                $taskIdentity = "$taskPid|$($taskStartTime.ToUniversalTime().Ticks)"
                [void]$taskLiveIdentities.Add($taskIdentity)
                $taskCpu = $taskCpuTime.TotalMilliseconds
                if ($taskPreviousCpu.ContainsKey($taskIdentity) -and $null -ne $taskPreviousUnixMs -and $taskNowUnixMs -gt $taskPreviousUnixMs) {
                    $taskRoles[$taskRole].cpuPercent += 100.0 * [Math]::Max(0, $taskCpu - $taskPreviousCpu[$taskIdentity]) / ($taskNowUnixMs - $taskPreviousUnixMs) / [Environment]::ProcessorCount
                }
                $taskPreviousCpu[$taskIdentity] = $taskCpu
                $taskRoles[$taskRole].processCount++
                $taskRoles[$taskRole].privateMiB += $taskChild.PrivateMemorySize64 / 1MB
                $taskRoles[$taskRole].workingSetMiB += $taskChild.WorkingSet64 / 1MB
            } catch [Microsoft.PowerShell.Commands.ProcessCommandException] {
                # 进程可能在采样期间正常退出；不会把旧 PID 的累计 CPU 移给新进程。
            } catch [System.ComponentModel.Win32Exception] {
                # 受保护服务进程可能不可读取；主诊断进程仍可持续采样。
            }
        }
        foreach ($taskIdentity in @($taskPreviousCpu.Keys)) {
            if (-not $taskLiveIdentities.Contains($taskIdentity)) { $taskPreviousCpu.Remove($taskIdentity) }
        }
        foreach ($taskRole in @('app','webview','engine','service')) {
            if ($taskRoles[$taskRole].processCount -eq 0) { $taskRoles.Remove($taskRole) }
        }
        $taskSample = [ordered]@{ timestampMs = $taskNowUnixMs; intervalStartUnixMs = $taskPreviousUnixMs; elapsedMs = [Math]::Round($taskClock.Elapsed.TotalMilliseconds); roles = $taskRoles }
        $taskWriter.WriteLine(($taskSample | ConvertTo-Json -Depth 6 -Compress))
        $taskWriter.Flush()
        $taskSampleCount++
        $taskPreviousUnixMs = $taskNowUnixMs
        Start-Sleep -Milliseconds $SampleIntervalMs
        $taskProcess.Refresh()
    }
    $taskWriter.Dispose()
    if (-not (Test-Path -LiteralPath $taskNativePath) -or (Get-Item -LiteralPath $taskNativePath).LastWriteTimeUtc -lt $taskStartedAt) { throw 'Native soak did not produce a fresh report.' }
    $taskMetadata = [ordered]@{ executableSha256 = $taskHash; timestamp = $taskStartedAt.ToString('o'); requestedDurationSeconds = $DurationSeconds; fixtureFiles = $FixtureFiles; sampleIntervalMs = $SampleIntervalMs; logicalProcessors = [Environment]::ProcessorCount; dedicatedServiceState = $taskServiceState; exitCode = $taskProcess.ExitCode }
    $taskMetadata | ConvertTo-Json | Set-Content -LiteralPath $taskMetadataPath -Encoding utf8
    & node (Join-Path $PSScriptRoot 'resource-summary.mjs') $taskSamplesPath $taskNativePath (Join-Path $taskReportRoot 'resource-soak.json') $taskMetadataPath
    if ($LASTEXITCODE -ne 0 -or $taskProcess.ExitCode -ne 0) { throw 'Native soak failed; inspect fresh reports.' }
} finally {
    $taskWriter.Dispose()
    if ($null -ne $taskProcess) {
        $taskProcess.Refresh()
        if (-not $taskProcess.HasExited) { Stop-Process -Id $taskProcess.Id }
    }
}
