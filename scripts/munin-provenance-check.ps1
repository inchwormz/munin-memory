param(
    [string]$MuninBin = "",
    [string]$RepoRoot = "",
    [ValidateSet("text", "json")]
    [string]$Format = "text",
    [switch]$RequireCandidate
)

$ErrorActionPreference = "Stop"

if (-not $RepoRoot) {
    $RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}
$RepoRoot = (Resolve-Path $RepoRoot).Path

if (-not $MuninBin) {
    $candidate = Join-Path $RepoRoot "target\debug\munin.exe"
    if (Test-Path $candidate) {
        $MuninBin = $candidate
    }
}
if (-not $MuninBin -or -not (Test-Path $MuninBin)) {
    Write-Error "Munin binary not found; pass -MuninBin <path>"
    exit 2
}
$MuninBin = (Resolve-Path $MuninBin).Path

function Read-JsonFile {
    param([string]$RelativePath)
    $path = Join-Path $RepoRoot $RelativePath
    if (-not (Test-Path $path)) {
        throw "required provenance file missing: $RelativePath"
    }
    return Get-Content $path -Raw | ConvertFrom-Json
}

function Hash-Artifact {
    param([string]$Path, [string]$Label)
    $resolved = (Resolve-Path $Path).Path
    $item = Get-Item $resolved
    [pscustomobject]@{
        label = $Label
        path = $resolved
        bytes = $item.Length
        sha256 = (Get-FileHash -Algorithm SHA256 $resolved).Hash.ToLowerInvariant()
    }
}

$manifestText = Get-Content (Join-Path $RepoRoot "Cargo.toml") -Raw
$manifestMatch = [regex]::Match($manifestText, '(?m)^version\s*=\s*"([^"]+)"\s*$')
if (-not $manifestMatch.Success) {
    throw "Cargo.toml package version not found"
}
$cargoVersion = $manifestMatch.Groups[1].Value
$claudePlugin = Read-JsonFile "plugins/munin-memory/.claude-plugin/plugin.json"
$codexPlugin = Read-JsonFile "plugins/munin-memory/.codex-plugin/plugin.json"

$binaryText = (& $MuninBin --version 2>&1 | Out-String).Trim()
$binaryExit = $LASTEXITCODE
$binaryMatch = [regex]::Match($binaryText, '(?m)^munin\s+([^\s]+)')
$binaryVersion = if ($binaryMatch.Success) { $binaryMatch.Groups[1].Value } else { "" }

$initialize = '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"munin-provenance-check","version":"1"}}}'
$mcpText = ($initialize | & $MuninBin mcp serve 2>$null | Out-String).Trim()
$mcpExit = $LASTEXITCODE
$mcpVersion = ""
try {
    $mcpMessages = @($mcpText -split "`r?`n" | Where-Object { $_.Trim() } | ForEach-Object { $_ | ConvertFrom-Json })
    $initializeResponse = @($mcpMessages | Where-Object id -eq 1 | Select-Object -First 1)
    if ($initializeResponse.Count -eq 1) {
        $mcpVersion = [string]$initializeResponse[0].result.serverInfo.version
    }
} catch {
    $mcpVersion = ""
}

$versions = [ordered]@{
    cargo = $cargoVersion
    binary = $binaryVersion
    claudePlugin = [string]$claudePlugin.version
    codexPlugin = [string]$codexPlugin.version
    mcpServer = $mcpVersion
}
$versionValues = @($versions.Values)
$versionsMatch = $binaryExit -eq 0 -and $mcpExit -eq 0 -and
    -not ($versionValues -contains "") -and
    @($versionValues | Select-Object -Unique).Count -eq 1

$commit = (& git -C $RepoRoot rev-parse HEAD 2>$null | Out-String).Trim()
$tags = @(& git -C $RepoRoot tag --points-at HEAD 2>$null | Where-Object { $_.Trim() })
$tagState = if ($tags.Count -eq 0) {
    "not created; approval required"
} else {
    "present: $($tags -join ', ')"
}

$artifacts = @(
    Hash-Artifact (Join-Path $RepoRoot "Cargo.lock") "cargo-lock"
    Hash-Artifact (Join-Path $RepoRoot "plugins/munin-memory/.claude-plugin/plugin.json") "claude-plugin"
    Hash-Artifact (Join-Path $RepoRoot "plugins/munin-memory/.codex-plugin/plugin.json") "codex-plugin"
    Hash-Artifact (Join-Path $RepoRoot ".claude-plugin/marketplace.json") "claude-marketplace"
    Hash-Artifact (Join-Path $RepoRoot ".agents/plugins/marketplace.json") "codex-marketplace"
    Hash-Artifact (Join-Path $RepoRoot "plugins/munin-memory/.mcp.json") "claude-mcp-declaration"
    Hash-Artifact (Join-Path $RepoRoot "plugins/munin-memory/.codex-mcp.json") "codex-mcp-declaration"
    Hash-Artifact $MuninBin "munin-binary"
)

$problems = New-Object System.Collections.Generic.List[string]
if (-not $versionsMatch) {
    $problems.Add("version surfaces differ or a binary/MCP probe failed")
}
if ($RequireCandidate -and $cargoVersion -ne "0.6.0-rc.1") {
    $problems.Add("candidate version must be 0.6.0-rc.1")
}
if ($RequireCandidate -and $tags.Count -ne 0) {
    $problems.Add("candidate tag already exists; tag creation is an approval boundary")
}

$receipt = [ordered]@{
    schema = "munin.provenance.v1"
    generatedAt = (Get-Date).ToUniversalTime().ToString("o")
    status = if ($problems.Count -eq 0) { "pass" } else { "fail" }
    commit = $commit
    versions = $versions
    versionsMatch = $versionsMatch
    candidateRequired = [bool]$RequireCandidate
    tag = $tagState
    artifacts = $artifacts
    problems = @($problems)
}

if ($Format -eq "json") {
    $receipt | ConvertTo-Json -Depth 8
} else {
    Write-Output "Munin Provenance"
    Write-Output "----------------"
    Write-Output ("Status: " + $receipt.status.ToUpperInvariant())
    Write-Output ("Commit: " + $receipt.commit)
    Write-Output ("Version: " + $cargoVersion)
    foreach ($entry in $versions.GetEnumerator()) {
        Write-Output ("  {0}: {1}" -f $entry.Key, $entry.Value)
    }
    Write-Output ("Tag: " + $tagState)
    Write-Output ("Artifacts hashed: " + $artifacts.Count)
    foreach ($problem in $problems) {
        Write-Output ("FAIL: " + $problem)
    }
}

if ($problems.Count -ne 0) {
    exit 1
}
exit 0
