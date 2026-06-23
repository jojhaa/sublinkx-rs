param(
  [string] $DatabaseUrl = "sqlite://data/app.db",
  [int] $BackendPort = 8080,
  [int] $FrontendPort = 5173,
  [switch] $NoPauseOnError
)

$ErrorActionPreference = "Stop"

$RootDir = Split-Path -Parent $PSScriptRoot
$BackendDir = Join-Path $RootDir "backend"
$FrontendDir = Join-Path $RootDir "frontend"
$BackendDataDir = Join-Path $BackendDir "data"
$LogDir = Join-Path $RootDir ".dev-logs"
$BackendLog = Join-Path $LogDir "backend.log"
$FrontendLog = Join-Path $LogDir "frontend.log"

function Test-Command($Name) {
  if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
    throw "Missing command '$Name'. Please install it first and make sure it is in PATH."
  }
}

function Get-PortOwnerSummary($Port) {
  $listeners = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue
  if (-not $listeners) {
    return $null
  }

  $owners = foreach ($listener in $listeners) {
    $process = Get-Process -Id $listener.OwningProcess -ErrorAction SilentlyContinue
    if ($process) {
      "$($process.ProcessName) (PID $($process.Id))"
    } else {
      "PID $($listener.OwningProcess)"
    }
  }

  return ($owners | Sort-Object -Unique) -join ", "
}

function Test-DevPort($Name, $Port) {
  $owner = Get-PortOwnerSummary $Port
  if ($owner) {
    throw "$Name port $Port is already in use by $owner. Stop that process or rerun with a different port."
  }
}

function Show-LogTail($Name, $Path) {
  if (-not (Test-Path $Path)) {
    Write-Host ""
    Write-Host "$Name log was not created: $Path"
    return
  }

  Write-Host ""
  Write-Host "Last $Name log lines ($Path):"
  Get-Content $Path -Tail 80
}

try {
  Test-Command cargo
  Test-Command npm

  Test-DevPort "Backend" $BackendPort
  Test-DevPort "Frontend" $FrontendPort

  New-Item -ItemType Directory -Force -Path $BackendDataDir | Out-Null
  New-Item -ItemType Directory -Force -Path $LogDir | Out-Null
  Remove-Item $BackendLog,$FrontendLog -Force -ErrorAction SilentlyContinue

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
  Write-Host "  Logs    : $LogDir"
  Write-Host ""
  Write-Host "Press Ctrl+C to stop both services."
  Write-Host ""

  $backendJob = Start-Job -Name "sublinkx-backend" -ArgumentList $BackendDir,$DatabaseUrl,$BackendPort,$BackendLog -ScriptBlock {
    param($BackendDir, $DatabaseUrl, $BackendPort, $BackendLog)
    Set-Location $BackendDir
    $env:APP_ENV = "development"
    $env:APP_PORT = "$BackendPort"
    $env:DATABASE_URL = $DatabaseUrl
    $env:SUBLINKX_RUNTIME_MODE = "local"
    $env:JWT_SECRET = "local-dev-secret-change-before-production"
    $env:BOOTSTRAP_ADMIN_USERNAME = "admin"
    $env:BOOTSTRAP_ADMIN_PASSWORD = "admin123456"
    cmd.exe /d /s /c "cargo run 2>&1" | Tee-Object -FilePath $BackendLog
    if ($LASTEXITCODE -ne 0) {
      exit $LASTEXITCODE
    }
  }

  $frontendJob = Start-Job -Name "sublinkx-frontend" -ArgumentList $FrontendDir,$FrontendPort,$BackendPort,$FrontendLog -ScriptBlock {
    param($FrontendDir, $FrontendPort, $BackendPort, $FrontendLog)
    Set-Location $FrontendDir
    $env:VITE_API_BASE_URL = "http://127.0.0.1:$BackendPort"
    cmd.exe /d /s /c "npm run dev -- --host 0.0.0.0 --port $FrontendPort 2>&1" | Tee-Object -FilePath $FrontendLog
    if ($LASTEXITCODE -ne 0) {
      exit $LASTEXITCODE
    }
  }

  try {
    while ($true) {
      Receive-Job -Job $backendJob,$frontendJob
      $backendState = (Get-Job -Id $backendJob.Id).State
      $frontendState = (Get-Job -Id $frontendJob.Id).State
      if ($backendState -ne "Running" -or $frontendState -ne "Running") {
        Receive-Job -Job $backendJob,$frontendJob
        Show-LogTail "backend" $BackendLog
        Show-LogTail "frontend" $FrontendLog
        throw "One of the local services stopped unexpectedly. Backend state: $backendState; frontend state: $frontendState."
      }
      Start-Sleep -Milliseconds 500
    }
  } finally {
    Write-Host ""
    Write-Host "Stopping local services..."
    Stop-Job -Job $backendJob,$frontendJob -ErrorAction SilentlyContinue
    Remove-Job -Job $backendJob,$frontendJob -Force -ErrorAction SilentlyContinue
  }
} catch {
  Write-Host ""
  Write-Host "Local development startup failed:" -ForegroundColor Red
  Write-Host "  $($_.Exception.Message)" -ForegroundColor Red
  if (-not $NoPauseOnError) {
    Write-Host ""
    Read-Host "Press Enter to close"
  }
  exit 1
}
