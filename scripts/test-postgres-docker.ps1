param(
    [string]$PostgresImage = 'postgres:16-alpine'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$projectRoot = Split-Path -Parent $PSScriptRoot
$backendDirectory = Join-Path $projectRoot 'backend'
$containerName = 'sublinkx-postgres-test-{0}-{1}' -f $PID, ([guid]::NewGuid().ToString('N').Substring(0, 8))
$databaseName = 'sublinkx_test'
$migrationDatabaseName = 'sublinkx_migration_test'
$databaseUser = 'sublinkx_test'
$databasePassword = [guid]::NewGuid().ToString('N')
$containerStarted = $false
$previousDatabaseUrl = $env:SUBLINKX_TEST_POSTGRES_URL
$previousMigrationDatabaseUrl = $env:SUBLINKX_TEST_POSTGRES_MIGRATION_URL

try {
    docker version *> $null
    if ($LASTEXITCODE -ne 0) {
        throw 'Docker is not available.'
    }

    docker run --detach `
        --name $containerName `
        --publish '127.0.0.1::5432' `
        --env "POSTGRES_DB=$databaseName" `
        --env "POSTGRES_USER=$databaseUser" `
        --env "POSTGRES_PASSWORD=$databasePassword" `
        $PostgresImage | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to start PostgreSQL image $PostgresImage."
    }
    $containerStarted = $true

    $ready = $false
    for ($attempt = 0; $attempt -lt 60; $attempt++) {
        docker exec $containerName pg_isready --host 127.0.0.1 --username $databaseUser --dbname $databaseName *> $null
        if ($LASTEXITCODE -eq 0) {
            $ready = $true
            break
        }
        Start-Sleep -Seconds 1
    }
    if (-not $ready) {
        throw 'PostgreSQL did not become ready within 60 seconds.'
    }

    docker exec $containerName createdb --host 127.0.0.1 --username $databaseUser $migrationDatabaseName
    if ($LASTEXITCODE -ne 0) {
        throw 'Failed to create the isolated PostgreSQL migration test database.'
    }

    $portOutput = docker port $containerName '5432/tcp'
    if ($LASTEXITCODE -ne 0 -or -not $portOutput) {
        throw 'Failed to resolve the temporary PostgreSQL host port.'
    }
    $hostPort = ($portOutput | Select-Object -First 1) -replace '^.*:', ''
    if ($hostPort -notmatch '^\d+$') {
        throw "Unexpected PostgreSQL port mapping: $portOutput"
    }

    $env:SUBLINKX_TEST_POSTGRES_URL = "postgresql://${databaseUser}:${databasePassword}@127.0.0.1:${hostPort}/${databaseName}"
    $env:SUBLINKX_TEST_POSTGRES_MIGRATION_URL = "postgresql://${databaseUser}:${databasePassword}@127.0.0.1:${hostPort}/${migrationDatabaseName}"
    Push-Location $backendDirectory
    try {
        cargo test --bin sublinkx-rs-backend 'db::tests::postgres_repository_smoke_when_configured' -- --exact --nocapture --test-threads=1
        if ($LASTEXITCODE -ne 0) {
            throw "PostgreSQL smoke test failed with exit code $LASTEXITCODE."
        }

        cargo test --bin sublinkx-rs-backend 'database_migration::tests::migrates_sqlite_to_postgres_when_configured' -- --exact --nocapture --test-threads=1
        if ($LASTEXITCODE -ne 0) {
            throw "PostgreSQL migration test failed with exit code $LASTEXITCODE."
        }
    }
    finally {
        Pop-Location
    }
}
finally {
    $env:SUBLINKX_TEST_POSTGRES_URL = $previousDatabaseUrl
    $env:SUBLINKX_TEST_POSTGRES_MIGRATION_URL = $previousMigrationDatabaseUrl
    if ($containerStarted) {
        docker rm --force $containerName *> $null
        if ($LASTEXITCODE -ne 0) {
            Write-Warning "Temporary container $containerName could not be removed."
        }
    }
}
