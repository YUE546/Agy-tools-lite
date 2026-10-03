# GitHub-hosted CI only. Read official runtime metadata, never browser profiles or credentials.
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'Native GUI acceptance is restricted to fresh GitHub-hosted runners.'
}
$runtimeRoot = Join-Path ${env:ProgramFiles(x86)} 'Microsoft\EdgeWebView\Application'
$runtime = Get-ChildItem -LiteralPath $runtimeRoot -Directory |
    Where-Object { $_.Name -match '^\d+\.\d+\.\d+\.\d+$' -and (Test-Path (Join-Path $_.FullName 'msedgewebview2.exe')) } |
    Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
if (!$runtime) { throw 'Installed Microsoft WebView2 Runtime not found; native acceptance cannot run.' }
$version = $runtime.Name
$driver = Join-Path $env:EDGEWEBDRIVER 'msedgedriver.exe'
$installedVersion = if (Test-Path -LiteralPath $driver) { (& $driver --version) -replace '^.*?(\d+\.\d+\.\d+\.\d+).*$', '$1' } else { '' }
$downloadRoot = $null
if ((($installedVersion -split '\.')[0..2] -join '.') -ne (($version -split '\.')[0..2] -join '.')) {
    $downloadRoot = Join-Path $env:RUNNER_TEMP ('agy-lite-edge-' + [guid]::NewGuid())
    New-Item -ItemType Directory -Path $downloadRoot | Out-Null
    try {
        $zip = Join-Path $downloadRoot 'driver.zip'
        # Microsoft official distribution endpoint, exact installed WebView2 version.
        Invoke-WebRequest -Uri "https://msedgedriver.microsoft.com/$version/edgedriver_win64.zip" -OutFile $zip
        Expand-Archive -LiteralPath $zip -DestinationPath $downloadRoot
        $driver = Join-Path $downloadRoot 'msedgedriver.exe'
        $signature = Get-AuthenticodeSignature -FilePath $driver
        if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation') {
            throw 'The downloaded Microsoft driver did not have a valid Microsoft signature.'
        }
        Remove-Item -LiteralPath $zip
    } catch {
        Remove-Item -LiteralPath $downloadRoot -Recurse -Force
        throw
    }
}
$infoPath = Join-Path $env:RUNNER_TEMP ('agy-lite-windows-driver-' + [guid]::NewGuid() + '.json')
@{
    is_elevated = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    driver = $driver; driver_version = (& $driver --version); driver_sha256 = (Get-FileHash $driver -Algorithm SHA256).Hash
    runtime = $runtime.FullName; runtime_version = $version; downloaded_driver_directory = $downloadRoot
    # dirs 5 uses Windows Known Folders. Changing HOME/APPDATA does NOT redirect these.
    known_home = [Environment]::GetFolderPath('UserProfile')
    known_roaming = [Environment]::GetFolderPath('ApplicationData')
    known_local = [Environment]::GetFolderPath('LocalApplicationData')
} | ConvertTo-Json | Set-Content -LiteralPath $infoPath -Encoding utf8
"GUI_WINDOWS_INFO=$infoPath" >> $env:GITHUB_ENV
"NATIVE_DRIVER=$driver" >> $env:GITHUB_ENV
