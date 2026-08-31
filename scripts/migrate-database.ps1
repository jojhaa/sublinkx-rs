param(
    [ValidateSet('check', 'run')]
    [string]$Mode = 'check',
    [switch]$ConfirmBackup
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if ([string]::IsNullOrWhiteSpace($env:SUBLINKX_MIGRATION_SOURCE_URL)) {
    throw 'Set SUBLINKX_MIGRATION_SOURCE_URL before running the migration.'
}
if ([string]::IsNullOrWhiteSpace($env:SUBLINKX_MIGRATION_TARGET_URL)) {
    throw 'Set SUBLINKX_MIGRATION_TARGET_URL before running the migration.'
}
if ($Mode -eq 'run' -and -not $ConfirmBackup) {
    throw 'Run mode requires -ConfirmBackup after creating and verifying a current backup.'
}

$projectRoot = Split-Path -Parent $PSScriptRoot
$backendDirectory = Join-Path $projectRoot 'backend'
$previousConfirmation = $env:SUBLINKX_MIGRATION_CONFIRM

try {
    if ($Mode -eq 'run') {
        $env:SUBLINKX_MIGRATION_CONFIRM = 'I_HAVE_A_CURRENT_BACKUP'
    }

    Push-Location $backendDirectory
    try {
        cargo run --locked -- migrate-database $Mode
        if ($LASTEXITCODE -ne 0) {
            throw "Database migration $Mode failed with exit code $LASTEXITCODE."
        }
    }
    finally {
        Pop-Location
    }
}
finally {
    $env:SUBLINKX_MIGRATION_CONFIRM = $previousConfirmation
}
