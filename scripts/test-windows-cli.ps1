# Real GUI-subsystem release executable; no OAuth, switching or live account data.
param([Parameter(Mandatory = $true)][string]$Binary, [switch]$Console)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$exe = (Resolve-Path -LiteralPath $Binary).Path
$bytes = [IO.File]::ReadAllBytes($exe)
$peOffset = [BitConverter]::ToInt32($bytes, 0x3c)
$subsystem = [BitConverter]::ToUInt16($bytes, $peOffset + 24 + 68)
if ($subsystem -ne $(if ($Console) { 3 } else { 2 })) {
    throw 'This check requires the Windows GUI-subsystem release executable, not the console debug build.'
}
$root = Join-Path ([IO.Path]::GetTempPath()) ('agy-lite-powershell-' + [guid]::NewGuid())
[IO.Directory]::CreateDirectory($root) | Out-Null
$previousData = $env:ABV_DATA_DIR
$env:ABV_DATA_DIR = Join-Path $root 'data'
$script:checks = 0

function Invoke-CapturedCli([string]$Arguments, [int]$ExpectedExitCode) {
    $stdout = Join-Path $root ("stdout-$script:checks.txt")
    $stderr = Join-Path $root ("stderr-$script:checks.txt")
    $process = Start-Process -FilePath $exe -ArgumentList $Arguments -NoNewWindow -Wait -PassThru `
        -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    if (-not $process.HasExited -or $null -eq $process.ExitCode -or $process.ExitCode -ne $ExpectedExitCode) {
        throw "Explicit wait returned an unexpected exit code for '$Arguments': $($process.ExitCode)"
    }
    $script:checks++
    return @{ stdout = [IO.File]::ReadAllText($stdout); stderr = [IO.File]::ReadAllText($stderr) }
}

try {
    if ($Console) {
        # PowerShell waits directly for a console executable and retains its exit code.
        $value = & $exe accounts list --json
        if ($LASTEXITCODE -ne 0 -or ($value | ConvertFrom-Json).accounts.Count -ne 0) { throw 'Direct PowerShell console call failed' }
        & $exe --unknown --json 2>$null
        if ($LASTEXITCODE -ne 2) { throw 'Direct PowerShell error exit code was lost' }
        $script:checks += 2
    }
    # Exercise the documented inherited-I/O invocation as well as redirection.
    # CI logs show its help text; GUI startup is never selected by this command.
    $process = Start-Process -FilePath $exe -ArgumentList '--help' -NoNewWindow -Wait -PassThru
    if (-not $process.HasExited -or $null -eq $process.ExitCode -or $process.ExitCode -ne 0) {
        throw 'Inherited-I/O help did not finish successfully.'
    }
    $script:checks++

    # Waiting only on the CLI process also supports switch callers, whose APP
    # child can outlive it. Validate that API here with a harmless read command.
    $process = Start-Process -FilePath $exe -ArgumentList '--version' -NoNewWindow -PassThru
    $process.WaitForExit()
    if (-not $process.HasExited -or $null -eq $process.ExitCode -or $process.ExitCode -ne 0) {
        throw 'Process-only wait did not preserve the exit code.'
    }
    $script:checks++

    $result = Invoke-CapturedCli 'accounts list --json' 0
    $json = $result.stdout | ConvertFrom-Json
    if ($json.schema_version -ne 1 -or $json.accounts.Count -ne 0 -or $result.stderr -ne '') {
        throw 'Captured account-list JSON was not isolated on stdout.'
    }
    foreach ($case in @(@{ arguments = 'current --json'; code = 3 }, @{ arguments = '--unknown --json'; code = 2 })) {
        $result = Invoke-CapturedCli $case.arguments $case.code
        $json = $result.stderr | ConvertFrom-Json
        if ($result.stdout -ne '' -or $json.schema_version -ne 1 -or $json.error.code -ne $case.code) {
            throw 'Captured error JSON or its exit code was not preserved.'
        }
    }
    if (Test-Path -LiteralPath $env:ABV_DATA_DIR) { throw 'Read-only commands created a data directory.' }
    Write-Output "$script:checks Windows release PowerShell wait/exit-code checks passed"
} finally {
    $env:ABV_DATA_DIR = $previousData
    Remove-Item -LiteralPath $root -Recurse -Force
}
