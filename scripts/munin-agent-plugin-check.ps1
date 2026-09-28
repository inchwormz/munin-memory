param(
    [Parameter(Mandatory = $true)]
    [string]$MuninBin
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Invoke-Checked {
    param(
        [Parameter(Mandatory = $true)][string]$Label,
        [Parameter(Mandatory = $true)][scriptblock]$Command
    )
    $output = & $Command 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) {
        throw "$Label failed with exit $LASTEXITCODE. $output"
    }
    return $output.Trim()
}

function Normalize-Json {
    param([Parameter(Mandatory = $true)][string]$Json)
    return ($Json | ConvertFrom-Json | ConvertTo-Json -Depth 100 -Compress)
}

function Assert-Contains {
    param(
        [Parameter(Mandatory = $true)][string]$Text,
        [Parameter(Mandatory = $true)][string]$Needle,
        [Parameter(Mandatory = $true)][string]$Label
    )
    if (-not $Text.Contains($Needle)) {
        throw "$Label did not contain '$Needle'. $Text"
    }
}

$muninPath = (Resolve-Path -LiteralPath $MuninBin).Path
$stamp = Get-Date -Format "yyyyMMdd-HHmmss-fff"
$checkBase = if ($env:MUNIN_AGENT_PLUGIN_CHECK_ROOT) {
    $env:MUNIN_AGENT_PLUGIN_CHECK_ROOT
} elseif ($IsWindows) {
    Join-Path ([Environment]::GetFolderPath("LocalApplicationData")) "Munin/checks"
} else {
    Join-Path ([System.IO.Path]::GetTempPath()) "munin-checks"
}
New-Item -ItemType Directory -Force $checkBase | Out-Null
$root = Join-Path $checkBase "munin-agent-plugin-check-$stamp-$PID"
$profile = Join-Path $root "profile"
$rollbackProfile = Join-Path $root "rollback-profile"
New-Item -ItemType Directory -Force (Join-Path $profile ".codex") | Out-Null
New-Item -ItemType Directory -Force (Join-Path $profile ".claude") | Out-Null
New-Item -ItemType Directory -Force (Join-Path $rollbackProfile ".codex") | Out-Null
New-Item -ItemType Directory -Force (Join-Path $rollbackProfile ".claude") | Out-Null
Set-Content -LiteralPath (Join-Path $profile ".codex/config.toml") -Value @("[features]", "plugins = true", "")
Set-Content -LiteralPath (Join-Path $rollbackProfile ".codex/config.toml") -Value @("[features]", "plugins = true", "")

$codexVersion = Invoke-Checked "Codex version" { codex --version }
$claudeVersion = Invoke-Checked "Claude version" { claude --version }

$env:MUNIN_INSTALL_HOME = $profile
$env:MUNIN_DATA_DIR = Join-Path $profile "data"
$env:MUNIN_CONFIG_DIR = Join-Path $profile "config"
$env:MUNIN_SESSION_HOME = Join-Path $profile "sessions"
$env:CODEX_HOME = Join-Path $profile ".codex"
$env:CLAUDE_CONFIG_DIR = Join-Path $profile ".claude"
Remove-Item Env:MUNIN_CODEX_BIN -ErrorAction SilentlyContinue
Remove-Item Env:MUNIN_CLAUDE_BIN -ErrorAction SilentlyContinue

$preCodexPlugins = Invoke-Checked "Codex pre plugin list" { codex plugin list --json }
$preCodexMarkets = Invoke-Checked "Codex pre marketplace list" { codex plugin marketplace list --json }
$preClaudePlugins = Invoke-Checked "Claude pre plugin list" { claude plugin list --json }
$preClaudeMarkets = Invoke-Checked "Claude pre marketplace list" { claude plugin marketplace list --json }

if ($IsWindows) {
    $failingClaude = Join-Path $root "claude-fail.cmd"
    @'
@echo off
if "%1 %2 %3"=="plugin marketplace list" (echo [] & exit /b 0)
if "%1 %2"=="plugin list" (echo [] & exit /b 0)
if "%1 %2"=="plugin validate" (echo Validation passed & exit /b 0)
if "%1 %2 %3"=="plugin marketplace add" (echo Marketplace added & exit /b 0)
if "%1 %2"=="plugin install" (echo forced Claude install failure 1>&2 & exit /b 7)
if "%1 %2 %3"=="plugin marketplace remove" (echo Marketplace removed & exit /b 0)
echo unexpected fake Claude arguments: %* 1>&2
exit /b 9
'@ | Set-Content -LiteralPath $failingClaude
} else {
    $failingClaude = Join-Path $root "claude-fail"
    @'
#!/bin/sh
if [ "$1 $2 $3" = "plugin marketplace list" ]; then echo '[]'; exit 0; fi
if [ "$1 $2" = "plugin list" ]; then echo '[]'; exit 0; fi
if [ "$1 $2" = "plugin validate" ]; then echo 'Validation passed'; exit 0; fi
if [ "$1 $2 $3" = "plugin marketplace add" ]; then echo 'Marketplace added'; exit 0; fi
if [ "$1 $2" = "plugin install" ]; then echo 'forced Claude install failure' >&2; exit 7; fi
if [ "$1 $2 $3" = "plugin marketplace remove" ]; then echo 'Marketplace removed'; exit 0; fi
echo "unexpected fake Claude arguments: $*" >&2
exit 9
'@ | Set-Content -LiteralPath $failingClaude -NoNewline
    chmod +x $failingClaude
}

$env:MUNIN_INSTALL_HOME = $rollbackProfile
$env:CODEX_HOME = Join-Path $rollbackProfile ".codex"
$env:CLAUDE_CONFIG_DIR = Join-Path $rollbackProfile ".claude"
$env:MUNIN_CLAUDE_BIN = $failingClaude
$rollbackOutput = & $muninPath plugin install --format json 2>&1 | Out-String
$rollbackExit = $LASTEXITCODE
if ($rollbackExit -eq 0) {
    throw "forced partial failure returned success. $rollbackOutput"
}
$rollbackCodexPlugins = Invoke-Checked "Codex rollback plugin list" { codex plugin list --json }
$rollbackCodexMarkets = Invoke-Checked "Codex rollback marketplace list" { codex plugin marketplace list --json }
if ($rollbackCodexPlugins.Contains("munin-memory") -or $rollbackCodexMarkets.Contains("munin-memory")) {
    throw "forced partial failure left Codex state behind. $rollbackCodexPlugins $rollbackCodexMarkets"
}

$env:MUNIN_INSTALL_HOME = $profile
$env:MUNIN_DATA_DIR = Join-Path $profile "data"
$env:MUNIN_CONFIG_DIR = Join-Path $profile "config"
$env:MUNIN_SESSION_HOME = Join-Path $profile "sessions"
$env:CODEX_HOME = Join-Path $profile ".codex"
$env:CLAUDE_CONFIG_DIR = Join-Path $profile ".claude"
Remove-Item Env:MUNIN_CLAUDE_BIN -ErrorAction SilentlyContinue

$installJson = Invoke-Checked "Munin plugin install" { & $muninPath plugin install --format json }
$install = $installJson | ConvertFrom-Json
if ($install.status -ne "pass" -or $install.hosts.Count -ne 2) {
    throw "Munin install receipt did not prove two passing hosts. $installJson"
}

$codexPlugins = Invoke-Checked "Codex plugin list" { codex plugin list --json }
$claudePlugins = Invoke-Checked "Claude plugin list" { claude plugin list --json }
$claudeDetails = Invoke-Checked "Claude plugin details" { claude plugin details munin-memory@munin-memory }
foreach ($nativeList in @($codexPlugins, $claudePlugins)) {
    Assert-Contains $nativeList "munin-memory@munin-memory" "native plugin list"
    Assert-Contains $nativeList "0.6.0-rc.2" "native plugin version"
    Assert-Contains $nativeList '"enabled": true' "native plugin enabled state"
}
Assert-Contains $claudeDetails "Skills" "Claude component inventory"
Assert-Contains $claudeDetails "Hooks" "Claude hook inventory"

$verifyJson = Invoke-Checked "Munin plugin verify" { & $muninPath plugin verify --format json }
$verify = $verifyJson | ConvertFrom-Json
if ($verify.status -ne "pass" -or $verify.hosts.Count -ne 2) {
    throw "Munin verify receipt did not prove two passing hosts. $verifyJson"
}

$stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
$hookJson = Invoke-Checked "Munin session-start hook" { & $muninPath hook session-start --format json }
$stopwatch.Stop()
$hook = $hookJson | ConvertFrom-Json
if ($hook.schema -ne "munin.agent-hook-receipt.v1" -or -not $hook.refresh_queued) {
    throw "session-start hook did not queue a versioned refresh. $hookJson"
}
if ($stopwatch.ElapsedMilliseconds -ge 1000) {
    throw "session-start hook exceeded local one-second gate: $($stopwatch.ElapsedMilliseconds) ms"
}

$workerStatusPath = Join-Path $profile "data/agent-hook/status.json"
$workerDeadline = (Get-Date).AddSeconds(30)
$workerStatus = $null
while ((Get-Date) -lt $workerDeadline) {
    if (Test-Path -LiteralPath $workerStatusPath) {
        $candidate = Get-Content -Raw -LiteralPath $workerStatusPath | ConvertFrom-Json
        if ($candidate.status -in @("pass", "fail")) {
            $workerStatus = $candidate
            break
        }
    }
    Start-Sleep -Milliseconds 50
}
if ($null -eq $workerStatus -or $workerStatus.status -ne "pass") {
    throw "incremental refresh worker did not finish successfully: $($workerStatus | ConvertTo-Json -Depth 20)"
}

$uninstallJson = Invoke-Checked "Munin plugin uninstall" { & $muninPath plugin uninstall --format json }
$uninstall = $uninstallJson | ConvertFrom-Json
if ($uninstall.status -ne "pass" -or $uninstall.hosts.Count -ne 2) {
    throw "Munin uninstall receipt did not prove two passing hosts. $uninstallJson"
}

$postCodexPlugins = Invoke-Checked "Codex post plugin list" { codex plugin list --json }
$postCodexMarkets = Invoke-Checked "Codex post marketplace list" { codex plugin marketplace list --json }
$postClaudePlugins = Invoke-Checked "Claude post plugin list" { claude plugin list --json }
$postClaudeMarkets = Invoke-Checked "Claude post marketplace list" { claude plugin marketplace list --json }
foreach ($pair in @(
    @($preCodexPlugins, $postCodexPlugins, "Codex plugins"),
    @($preCodexMarkets, $postCodexMarkets, "Codex marketplaces"),
    @($preClaudePlugins, $postClaudePlugins, "Claude plugins"),
    @($preClaudeMarkets, $postClaudeMarkets, "Claude marketplaces")
)) {
    if ((Normalize-Json $pair[0]) -ne (Normalize-Json $pair[1])) {
        throw "$($pair[2]) post-state differs from pre-state. PRE: $($pair[0]) POST: $($pair[1])"
    }
}

Write-Output "[PASS] Native plugin lifecycle"
Write-Output "  Munin: $muninPath"
Write-Output "  Codex: $codexVersion"
Write-Output "  Claude: $claudeVersion"
Write-Output "  Identity: munin-memory@munin-memory 0.6.0-rc.2"
Write-Output "  Hosts verified: 2"
Write-Output "  Forced rollback exit: $rollbackExit"
Write-Output "  Session-start latency: $($stopwatch.ElapsedMilliseconds) ms"
Write-Output "  Worker status: $($workerStatus.status)"
Write-Output "  Receipt root: $root"
