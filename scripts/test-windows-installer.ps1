# Fresh hosted CI only. Test per-user NSIS install without launching the GUI.
param([Parameter(Mandatory = $true)][string]$Installer)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') { throw 'Use a fresh GitHub-hosted runner' }
$package = (Resolve-Path -LiteralPath $Installer).Path
$root = Join-Path $env:RUNNER_TEMP ('agy-installer-' + [guid]::NewGuid())
if (Test-Path -LiteralPath $root) { throw 'Installation destination already exists' }
$previousData = $env:ABV_DATA_DIR
$env:ABV_DATA_DIR = Join-Path $root 'isolated-data'
try {
    $process = Start-Process -FilePath $package -ArgumentList "/S /D=$root" -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw "NSIS installation failed: $($process.ExitCode)" }
    $cli = Join-Path $root 'agy-switch.exe'
    if (!(Test-Path -LiteralPath $cli)) { throw 'Installer did not include the console CLI' }
    $json = & $cli accounts list --json
    if ($LASTEXITCODE -ne 0 -or ($json | ConvertFrom-Json).accounts.Count -ne 0) { throw 'Installed CLI failed' }
    & $cli --version
    if ($LASTEXITCODE -ne 0) { throw 'Installed CLI version failed' }
    if (Test-Path -LiteralPath $env:ABV_DATA_DIR) { throw 'Read commands created account data' }
    Write-Output 'Actual NSIS installation and installed console CLI passed; no GUI/auth/account writes'
} finally {
    $env:ABV_DATA_DIR = $previousData
    $uninstaller = Join-Path $root 'uninstall.exe'
    if (Test-Path -LiteralPath $uninstaller) {
        $process = Start-Process -FilePath $uninstaller -ArgumentList '/S' -Wait -PassThru
        if ($process.ExitCode -ne 0) { throw "Test uninstall failed: $($process.ExitCode)" }
    }
    # Only the unique installation directory owned by this fixture.
    if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
}
