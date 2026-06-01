param(
  [string] $DatabaseUrl = "sqlite://data/app.db",
  [int] $BackendPort = 8080,
  [int] $FrontendPort = 5173
)

$ErrorActionPreference = "Stop"

$RootDir = Split-Path -Parent $PSScriptRoot
$BackendDir = Join-Path $RootDir "backend"
$FrontendDir = Join-Path $RootDir "frontend"
$BackendDataDir = Join-Path $BackendDir "data"

function Test-Command($Name) {
  if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
    throw "Missing command '$Name'. Please install it first and make sure it is in PATH."
  }
}

Test-Command cargo
Test-Command npm

New-Item -ItemType Directory -Force -Path $BackendDataDir | Out-Null

if (-not (Test-Path (Join-Path $FrontendDir "node_modules"))) {
  Write-Host "Installing frontend dependencies..."
  Push-Location $FrontendDir
  npm install
  Pop-Location
}

Write-Host ""
Write-Host "Starting sublinkx-rs locally..."
Write-Host "  Backend : http://127.0.0.1:$BackendPort"
Write-Host "  Frontend: http://127.0.0.1:$FrontendPort"
Write-Host "  Database: $DatabaseUrl"
Write-Host ""
Write-Host "Press Ctrl+C to stop both services."
Write-Host ""

$backendJob = Start-Job -Name "sublinkx-backend" -ArgumentList $BackendDir,$DatabaseUrl,$BackendPort -ScriptBlock {
  param($BackendDir, $DatabaseUrl, $BackendPort)
  Set-Location $BackendDir
  $env:APP_ENV = "development"
  $env:APP_PORT = "$BackendPort"
  $env:DATABASE_URL = $DatabaseUrl
  $env:SUBLINKX_RUNTIME_MODE = "local"
  $env:JWT_SECRET = "local-dev-secret-change-before-production"
  $env:BOOTSTRAP_ADMIN_USERNAME = "admin"
  $env:BOOTSTRAP_ADMIN_PASSWORD = "admin123456"
  cargo run
}

$frontendJob = Start-Job -Name "sublinkx-frontend" -ArgumentList $FrontendDir,$FrontendPort,$BackendPort -ScriptBlock {
  param($FrontendDir, $FrontendPort, $BackendPort)
  Set-Location $FrontendDir
  $env:VITE_API_BASE_URL = "http://127.0.0.1:$BackendPort"
  npm run dev -- --host 0.0.0.0 --port $FrontendPort
}

try {
  while ($true) {
    Receive-Job -Job $backendJob,$frontendJob
    if ($backendJob.State -ne "Running" -or $frontendJob.State -ne "Running") {
      Receive-Job -Job $backendJob,$frontendJob
      throw "One of the local services stopped unexpectedly."
    }
    Start-Sleep -Milliseconds 500
  }
} finally {
  Write-Host ""
  Write-Host "Stopping local services..."
  Stop-Job -Job $backendJob,$frontendJob -ErrorAction SilentlyContinue
  Remove-Job -Job $backendJob,$frontendJob -Force -ErrorAction SilentlyContinue
}
