param(
    [string]$MuninBin = "",
    [string]$RepoRoot = "",
    [switch]$NativeMcpHosts
)

$ErrorActionPreference = "Stop"

if (-not $RepoRoot) {
    $RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}

if (-not $MuninBin) {
    $debugBin = Join-Path $RepoRoot "target\debug\munin.exe"
    if (Test-Path $debugBin) {
        $MuninBin = $debugBin
    } else {
        $cmd = Get-Command munin -ErrorAction SilentlyContinue
        if ($cmd) {
            $MuninBin = $cmd.Source
        }
    }
}

if ($MuninBin -and (Test-Path $MuninBin)) {
    $MuninBin = (Resolve-Path $MuninBin).Path
}

$results = New-Object System.Collections.Generic.List[object]

function Add-Result {
    param(
        [string]$Name,
        [string]$Status,
        [string]$Detail
    )
    $script:results.Add([pscustomobject]@{
        Name = $Name
        Status = $Status
        Detail = $Detail
    }) | Out-Null
}

function Run-Munin {
    param(
        [string]$Name,
        [string[]]$CliArgs,
        [string]$WorkingDirectory,
        [scriptblock]$Judge
    )

    Push-Location $WorkingDirectory
    try {
        $oldErrorActionPreference = $ErrorActionPreference
        $ErrorActionPreference = "Continue"
        $output = (& $MuninBin @CliArgs 2>&1 | Out-String)
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $oldErrorActionPreference
        Pop-Location
    }

    & $Judge $Name $exitCode $output ""
}

function First-Line {
    param([string]$Text)
    $line = ($Text -split "`r?`n" | Where-Object { $_.Trim() } | Select-Object -First 1)
    if ($line) { return $line.Trim() }
    return ""
}

function Run-McpProtocolSmoke {
    param([string]$WorkingDirectory)
    $requests = @(
        '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"munin-release-check","version":"1"}}}',
        '{"jsonrpc":"2.0","method":"notifications/initialized"}',
        '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
    )
    Push-Location $WorkingDirectory
    try {
        $oldErrorActionPreference = $ErrorActionPreference
        $ErrorActionPreference = "Continue"
        $output = ($requests | & $MuninBin mcp serve 2>&1 | Out-String)
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $oldErrorActionPreference
        Pop-Location
    }

    if ($exitCode -ne 0) {
        Add-Result "MCP protocol smoke" "FAIL" (First-Line $output)
        return
    }
    try {
        $messages = @($output -split "`r?`n" | Where-Object { $_.Trim() } | ForEach-Object { $_ | ConvertFrom-Json })
        $toolResponse = @($messages | Where-Object id -eq 2)
        if ($toolResponse.Count -ne 1) {
            throw "expected one tools/list response"
        }
        $tools = @($toolResponse[0].result.tools)
        $actual = @($tools | ForEach-Object name | Sort-Object)
        $expected = @("friction", "health", "project_state", "search", "session_summary", "strategy_status")
        if (@(Compare-Object $expected $actual).Count -ne 0) {
            throw "tool set differs: $($actual -join ', ')"
        }
        if (@($tools | Where-Object { $_.annotations.readOnlyHint -ne $true }).Count -ne 0) {
            throw "one or more MCP tools are not marked read-only"
        }
        Add-Result "MCP protocol smoke" "PASS" "Six strict read-only tools advertised over JSON-RPC."
    } catch {
        Add-Result "MCP protocol smoke" "FAIL" "$($_.Exception.Message) Output: $(First-Line $output)"
    }
}

Write-Output "Munin Release Check"
Write-Output "-------------------"

if (-not $MuninBin -or -not (Test-Path $MuninBin)) {
    Add-Result "Binary" "FAIL" "Munin binary was not found. Build with 'cargo build --bin munin' or install Munin first."
} else {
    Add-Result "Binary" "PASS" $MuninBin

    Run-Munin "Install contract from repo" @("install", "--check-resolvable") $RepoRoot {
        param($Name, $ExitCode, $Stdout, $Stderr)
        if ($ExitCode -eq 0 -and $Stdout -match "resolver, skill, and fixture checks passed") {
            Add-Result $Name "PASS" (First-Line $Stdout)
        } else {
            Add-Result $Name "FAIL" (First-Line ($Stderr + "`n" + $Stdout))
        }
    }

    $tempCwd = Join-Path ([System.IO.Path]::GetTempPath()) ("munin-release-check-" + [guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Force -Path $tempCwd | Out-Null
    try {
        Run-Munin "Install contract from temp folder" @("install", "--check-resolvable") $tempCwd {
            param($Name, $ExitCode, $Stdout, $Stderr)
            if ($ExitCode -eq 0 -and $Stdout -match "resolver, skill, and fixture checks passed") {
                Add-Result $Name "PASS" (First-Line $Stdout)
            } else {
                Add-Result $Name "FAIL" (First-Line ($Stderr + "`n" + $Stdout))
            }
        }
    } finally {
        Remove-Item -LiteralPath $tempCwd -Recurse -Force -ErrorAction SilentlyContinue
    }

    Run-Munin "Memory health" @("doctor", "--scope", "user", "--format", "text") $RepoRoot {
        param($Name, $ExitCode, $Stdout, $Stderr)
        if ($ExitCode -ne 0) {
            Add-Result $Name "FAIL" (First-Line ($Stderr + "`n" + $Stdout))
        } elseif ($Stdout -match "Status: warn") {
            Add-Result $Name "WARN" "Doctor reports warn. Check the recommended permanent fix."
        } elseif ($Stdout -match "Status: pass|Status: ok|Status: healthy") {
            Add-Result $Name "PASS" (First-Line $Stdout)
        } else {
            Add-Result $Name "WARN" "Doctor ran, but status was not recognised."
        }
    }

    Run-Munin "Promotion proof" @("prove", "--format", "text") $RepoRoot {
        param($Name, $ExitCode, $Stdout, $Stderr)
        if ($ExitCode -ne 0) {
            Add-Result $Name "FAIL" (First-Line ($Stderr + "`n" + $Stdout))
        } elseif ($Stdout -match "Decision: .*passed|Strict promotion gate passed") {
            Add-Result $Name "PASS" "Promotion proof passed."
        } elseif ($Stdout -match "blocked|missing") {
            Add-Result $Name "WARN" "Promotion proof is blocked or missing required rows."
        } else {
            Add-Result $Name "WARN" "Proof ran, but decision was not recognised."
        }
    }

    Run-Munin "Resolver routing" @("resolve", "--format", "text", "what", "keeps", "going", "wrong") $RepoRoot {
        param($Name, $ExitCode, $Stdout, $Stderr)
        if ($ExitCode -eq 0 -and $Stdout -match "Route: friction") {
            Add-Result $Name "PASS" "Plain-English friction question routes correctly."
        } else {
            Add-Result $Name "FAIL" (First-Line ($Stderr + "`n" + $Stdout))
        }
    }

    Run-McpProtocolSmoke $RepoRoot

    if ($NativeMcpHosts) {
        $hostCheck = Join-Path $RepoRoot "scripts/munin-mcp-host-check.ps1"
        $oldErrorActionPreference = $ErrorActionPreference
        $ErrorActionPreference = "Continue"
        $hostOutput = (& $hostCheck -MuninBin $MuninBin 2>&1 | Out-String)
        $hostExit = $LASTEXITCODE
        $ErrorActionPreference = $oldErrorActionPreference
        if ($hostExit -eq 0 -and $hostOutput -match "\[PASS\] Native MCP host integration") {
            Add-Result "Native MCP hosts" "PASS" "Broken-runtime falsifier and real Claude/Codex tool call passed."
        } else {
            Add-Result "Native MCP hosts" "FAIL" (First-Line $hostOutput)
        }
    } else {
        Add-Result "Native MCP hosts" "SKIP" "Run with -NativeMcpHosts before a host-compatibility release."
    }
}

foreach ($result in $results) {
    Write-Output ("[{0}] {1} - {2}" -f $result.Status, $result.Name, $result.Detail)
}

if ($results.Status -contains "FAIL") {
    Write-Output "Next action: fix the FAIL items before trusting this release."
    exit 1
}

if ($results.Status -contains "WARN") {
    Write-Output "Next action: fix the WARN items before trusting this release."
    exit 1
}

Write-Output "Next action: release check is green."
exit 0
