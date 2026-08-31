param(
    [string]$MySqlImage = 'mysql:8.4'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$projectRoot = Split-Path -Parent $PSScriptRoot
$backendDirectory = Join-Path $projectRoot 'backend'
$containerName = 'sublinkx-mysql-test-{0}-{1}' -f $PID, ([guid]::NewGuid().ToString('N').Substring(0, 8))
$databaseName = 'sublinkx_migration_test'
$databaseUser = 'sublinkx_test'
$databasePassword = [guid]::NewGuid().ToString('N')
$rootPassword = [guid]::NewGuid().ToString('N')
$containerStarted = $false
$previousMigrationDatabaseUrl = $env:SUBLINKX_TEST_MYSQL_MIGRATION_URL

try {
    docker version *> $null
    if ($LASTEXITCODE -ne 0) {
        throw 'Docker is not available.'
    }

    docker run --detach `
        --name $containerName `
        --publish '127.0.0.1::3306' `
        --env "MYSQL_DATABASE=$databaseName" `
        --env "MYSQL_USER=$databaseUser" `
        --env "MYSQL_PASSWORD=$databasePassword" `
        --env "MYSQL_ROOT_PASSWORD=$rootPassword" `
        $MySqlImage | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to start MySQL image $MySqlImage."
    }
    $containerStarted = $true

    $ready = $false
    for ($attempt = 0; $attempt -lt 120; $attempt++) {
        docker exec $containerName mysqladmin ping --host 127.0.0.1 --user $databaseUser "--password=$databasePassword" --silent *> $null
        if ($LASTEXITCODE -eq 0) {
            $ready = $true
            break
        }
        Start-Sleep -Seconds 1
    }
    if (-not $ready) {
        throw 'MySQL did not become ready within 120 seconds.'
    }

    $portOutput = docker port $containerName '3306/tcp'
    if ($LASTEXITCODE -ne 0 -or -not $portOutput) {
        throw 'Failed to resolve the temporary MySQL host port.'
    }
    $hostPort = ($portOutput | Select-Object -First 1) -replace '^.*:', ''
    if ($hostPort -notmatch '^\d+$') {
        throw "Unexpected MySQL port mapping: $portOutput"
    }

    $env:SUBLINKX_TEST_MYSQL_MIGRATION_URL = "mysql://${databaseUser}:${databasePassword}@127.0.0.1:${hostPort}/${databaseName}"
    Push-Location $backendDirectory
    try {
        cargo test --bin sublinkx-rs-backend 'database_migration::tests::migrates_sqlite_to_mysql_and_back_when_configured' -- --exact --nocapture --test-threads=1
        if ($LASTEXITCODE -ne 0) {
            throw "MySQL migration test failed with exit code $LASTEXITCODE."
        }
    }
    finally {
        Pop-Location
    }
}
finally {
    $env:SUBLINKX_TEST_MYSQL_MIGRATION_URL = $previousMigrationDatabaseUrl
    if ($containerStarted) {
        docker rm --force $containerName *> $null
        if ($LASTEXITCODE -ne 0) {
            Write-Warning "Temporary container $containerName could not be removed."
        }
    }
}
