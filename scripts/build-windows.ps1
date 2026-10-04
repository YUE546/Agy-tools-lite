# Build both executables before referencing the console program as a bundle resource.
$ErrorActionPreference = 'Stop'
Push-Location (Join-Path $PSScriptRoot '..')
try {
    npm ci
    if ($LASTEXITCODE -ne 0) { throw 'Frontend dependencies failed' }
    npm run tauri build -- --no-bundle -- --locked
    if ($LASTEXITCODE -ne 0) { throw 'Windows build failed' }
    npm run tauri bundle -- --bundles nsis --config src-tauri/tauri.windows.bundle.conf.json
    if ($LASTEXITCODE -ne 0) { throw 'Windows installer packaging failed' }
} finally { Pop-Location }
