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

function Start-CodexAppServer {
    param([Parameter(Mandatory = $true)][string]$RuntimePath)
    $info = [System.Diagnostics.ProcessStartInfo]::new()
    if ($IsWindows) {
        $codexCommand = (Get-Command codex.cmd -ErrorAction Stop).Source
        $info.FileName = $env:ComSpec
        $info.Arguments = "/d /s /c `"`"$codexCommand`" app-server --enable plugins --stdio`""
    } else {
        $info.FileName = (Get-Command codex -ErrorAction Stop).Source
        $info.ArgumentList.Add("app-server")
        $info.ArgumentList.Add("--enable")
        $info.ArgumentList.Add("plugins")
        $info.ArgumentList.Add("--stdio")
    }
    $info.UseShellExecute = $false
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.CreateNoWindow = $true
    $info.Environment["PATH"] = "$RuntimePath$([IO.Path]::PathSeparator)$script:OriginalPath"

    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $info
    [void]$process.Start()
    $process.StandardInput.AutoFlush = $true
    return [pscustomobject]@{
        Process = $process
        ErrorTask = $process.StandardError.ReadToEndAsync()
    }
}

function Write-AppServerMessage {
    param(
        [Parameter(Mandatory = $true)][System.Diagnostics.Process]$Process,
        [Parameter(Mandatory = $true)][hashtable]$Message
    )
    $Process.StandardInput.WriteLine(($Message | ConvertTo-Json -Compress -Depth 20))
}

function Read-AppServerResponse {
    param(
        [Parameter(Mandatory = $true)][System.Diagnostics.Process]$Process,
        [Parameter(Mandatory = $true)][int]$Id
    )
    while ($true) {
        $read = $Process.StandardOutput.ReadLineAsync()
        if (-not $read.Wait(15000)) {
            throw "Codex app-server timed out waiting for response id $Id"
        }
        $line = $read.Result
        if ($null -eq $line) {
            throw "Codex app-server closed before response id $Id"
        }
        $message = $line | ConvertFrom-Json
        if ($message.PSObject.Properties.Name -contains "id" -and
            $null -ne $message.id -and [int]$message.id -eq $Id) {
            return $message
        }
    }
}

function Stop-CodexAppServer {
    param([Parameter(Mandatory = $true)]$Session)
    if (-not $Session.Process.HasExited) {
        $Session.Process.StandardInput.Close()
        if (-not $Session.Process.WaitForExit(5000)) {
            $Session.Process.Kill($true)
            $Session.Process.WaitForExit()
        }
    }
    return $Session.ErrorTask.Result
}

function Invoke-CodexMcpProbe {
    param(
        [Parameter(Mandatory = $true)][string]$RuntimePath,
        [Parameter(Mandatory = $true)][bool]$ExpectConnected
    )
    $session = Start-CodexAppServer $RuntimePath
    $stderr = ""
    try {
        Write-AppServerMessage $session.Process @{
            id = 1
            method = "initialize"
            params = @{ clientInfo = @{ name = "munin-mcp-host-check"; version = "1" } }
        }
        $initialize = Read-AppServerResponse $session.Process 1
        if ($initialize.PSObject.Properties.Name -contains "error") {
            throw "Codex initialize failed: $($initialize | ConvertTo-Json -Depth 20)"
        }
        Write-AppServerMessage $session.Process @{ method = "initialized" }
        Write-AppServerMessage $session.Process @{
            id = 2
            method = "mcpServerStatus/list"
            params = @{ detail = "full" }
        }
        $status = Read-AppServerResponse $session.Process 2
        $server = @($status.result.data | Where-Object name -eq "munin-memory")
        if ($server.Count -ne 1) {
            throw "Codex did not expose exactly one munin-memory server. $($status | ConvertTo-Json -Depth 30)"
        }

        $toolNames = @($server[0].tools.PSObject.Properties | ForEach-Object Name | Sort-Object)
        if (-not $ExpectConnected) {
            if ($null -ne $server[0].serverInfo -or $toolNames.Count -ne 0) {
                throw "Broken Munin runtime unexpectedly exposed a connected Codex MCP server. $($status | ConvertTo-Json -Depth 30)"
            }
            return [pscustomobject]@{ ToolNames = $toolNames; HealthSchema = $null; ServerVersion = $null }
        }

        $expected = @("friction", "health", "project_state", "search", "session_summary", "strategy_status")
        if (@(Compare-Object $expected $toolNames).Count -ne 0) {
            throw "Codex MCP tool surface differs from the six-tool contract. Actual: $($toolNames -join ', ')"
        }
        if ($server[0].serverInfo.version -ne "0.6.0-rc.1") {
            throw "Codex MCP server version mismatch: $($server[0].serverInfo.version)"
        }

        Write-AppServerMessage $session.Process @{
            id = 3
            method = "thread/start"
            params = @{ cwd = (Get-Location).Path; ephemeral = $true }
        }
        $thread = Read-AppServerResponse $session.Process 3
        if ($thread.PSObject.Properties.Name -contains "error") {
            throw "Codex host-check thread failed: $($thread | ConvertTo-Json -Depth 20)"
        }
        Write-AppServerMessage $session.Process @{
            id = 4
            method = "mcpServer/tool/call"
            params = @{
                server = "munin-memory"
                tool = "health"
                arguments = @{}
                threadId = $thread.result.thread.id
            }
        }
        $health = Read-AppServerResponse $session.Process 4
        if ($health.PSObject.Properties.Name -contains "error" -or $health.result.isError -ne $false) {
            throw "Codex health tool call failed: $($health | ConvertTo-Json -Depth 30)"
        }
        if ($health.result.structuredContent.schemaVersion -ne "munin.mcp.result.v1" -or
            $health.result.structuredContent.tool -ne "health") {
            throw "Codex health tool returned the wrong typed contract. $($health | ConvertTo-Json -Depth 30)"
        }
        return [pscustomobject]@{
            ToolNames = $toolNames
            HealthSchema = $health.result.structuredContent.schemaVersion
            ServerVersion = $server[0].serverInfo.version
        }
    } finally {
        $stderr = Stop-CodexAppServer $session
        if ($ExpectConnected -and $stderr.Contains("Failed to list resources for MCP server 'munin-memory'")) {
            throw "Codex emitted a Munin MCP discovery warning. $stderr"
        }
    }
}

$muninPath = (Resolve-Path -LiteralPath $MuninBin).Path
$script:OriginalPath = $env:PATH
$stamp = Get-Date -Format "yyyyMMdd-HHmmss-fff"
$checkBase = if ($env:MUNIN_MCP_HOST_CHECK_ROOT) {
    $env:MUNIN_MCP_HOST_CHECK_ROOT
} elseif ($IsWindows) {
    Join-Path ([Environment]::GetFolderPath("LocalApplicationData")) "Munin/checks"
} else {
    Join-Path ([System.IO.Path]::GetTempPath()) "munin-checks"
}
New-Item -ItemType Directory -Force $checkBase | Out-Null
$root = Join-Path $checkBase "munin-mcp-host-check-$stamp-$PID"
$profile = Join-Path $root "profile"
$realBin = Join-Path $root "bin"
$failingBin = Join-Path $root "failing-bin"
New-Item -ItemType Directory -Force (Join-Path $profile ".codex") | Out-Null
New-Item -ItemType Directory -Force (Join-Path $profile ".claude") | Out-Null
New-Item -ItemType Directory -Force $realBin | Out-Null
New-Item -ItemType Directory -Force $failingBin | Out-Null
Set-Content -LiteralPath (Join-Path $profile ".codex/config.toml") -Value @("[features]", "plugins = true", "")

if ($IsWindows) {
    Copy-Item -LiteralPath $muninPath -Destination (Join-Path $realBin "munin.exe")
    Copy-Item -LiteralPath (Join-Path $env:SystemRoot "System32/where.exe") -Destination (Join-Path $failingBin "munin.exe")
} else {
    Copy-Item -LiteralPath $muninPath -Destination (Join-Path $realBin "munin")
    Set-Content -LiteralPath (Join-Path $failingBin "munin") -NoNewline -Value "#!/bin/sh`nexit 127`n"
    chmod +x (Join-Path $realBin "munin") (Join-Path $failingBin "munin")
}

$env:MUNIN_INSTALL_HOME = $profile
$env:MUNIN_DATA_DIR = Join-Path $profile "data"
$env:MUNIN_CONFIG_DIR = Join-Path $profile "config"
$env:MUNIN_SESSION_HOME = Join-Path $profile "sessions"
$env:CODEX_HOME = Join-Path $profile ".codex"
$env:CLAUDE_CONFIG_DIR = Join-Path $profile ".claude"
$env:PATH = "$realBin$([IO.Path]::PathSeparator)$script:OriginalPath"
Remove-Item Env:MUNIN_CODEX_BIN -ErrorAction SilentlyContinue
Remove-Item Env:MUNIN_CLAUDE_BIN -ErrorAction SilentlyContinue

$codexVersion = Invoke-Checked "Codex version" { codex --version }
$claudeVersion = Invoke-Checked "Claude version" { claude --version }
$preCodexPlugins = Invoke-Checked "Codex pre plugin list" { codex plugin list --json }
$preCodexMarkets = Invoke-Checked "Codex pre marketplace list" { codex plugin marketplace list --json }
$preClaudePlugins = Invoke-Checked "Claude pre plugin list" { claude plugin list --json }
$preClaudeMarkets = Invoke-Checked "Claude pre marketplace list" { claude plugin marketplace list --json }

$installJson = Invoke-Checked "Munin plugin install" { & $muninPath plugin install --format json }
$install = $installJson | ConvertFrom-Json
if ($install.status -ne "pass" -or $install.hosts.Count -ne 2) {
    throw "Munin install receipt did not prove two passing hosts. $installJson"
}

$claudePlugins = Invoke-Checked "Claude plugin list" { claude plugin list --json }
$claudePlugin = @($claudePlugins | ConvertFrom-Json | Where-Object id -eq "munin-memory@munin-memory")
$claudeErrors = @()
if ($claudePlugin.Count -eq 1 -and $claudePlugin[0].PSObject.Properties.Name -contains "errors") {
    $claudeErrors = @($claudePlugin[0].errors | Where-Object { $null -ne $_ })
}
if ($claudePlugin.Count -ne 1 -or $claudeErrors.Count -ne 0) {
    throw "Claude plugin inventory contains errors. $claudePlugins"
}

$env:PATH = "$failingBin$([IO.Path]::PathSeparator)$script:OriginalPath"
$brokenClaude = & claude mcp get plugin:munin-memory:munin-memory 2>&1 | Out-String
if ($brokenClaude -notmatch "Failed to connect|Disconnected") {
    throw "Broken Munin runtime did not make Claude's MCP probe fail. $brokenClaude"
}
$brokenCodex = Invoke-CodexMcpProbe -RuntimePath $failingBin -ExpectConnected $false

$env:PATH = "$realBin$([IO.Path]::PathSeparator)$script:OriginalPath"
$claudeMcp = Invoke-Checked "Claude MCP connection" { claude mcp get plugin:munin-memory:munin-memory }
Assert-Contains $claudeMcp "Connected" "Claude MCP connection"
$codexMcp = Invoke-CodexMcpProbe -RuntimePath $realBin -ExpectConnected $true

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

Write-Output "[PASS] Native MCP host integration"
Write-Output "  Munin: $muninPath"
Write-Output "  Codex: $codexVersion"
Write-Output "  Claude: $claudeVersion"
Write-Output "  False-green falsifier: Claude disconnected; Codex tools=$($brokenCodex.ToolNames.Count)"
Write-Output "  Codex tools: $($codexMcp.ToolNames -join ', ')"
Write-Output "  Codex health schema: $($codexMcp.HealthSchema)"
Write-Output "  Claude MCP: Connected"
Write-Output "  Receipt root: $root"
