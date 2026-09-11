param([Parameter(Mandatory = $true)][string]$Version)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw 'Invalid version' }
$suffix = [guid]::NewGuid().ToString('N').Substring(0, 10)
$network = "sublinkx-smoke-$suffix"
$backend = "$network-backend"
$frontend = "$network-frontend"
function DockerChecked {
  param([string[]]$Arguments)
  $result = & docker @Arguments
  if ($LASTEXITCODE -ne 0) { throw "Docker failed: $($Arguments[0])" }
  return $result
}
try {
  DockerChecked @('network', 'create', $network) | Out-Null
  DockerChecked @('run', '-d', '--name', $backend, '--network', $network,
    '--network-alias', 'sublinkx-backend', '--tmpfs', '/app/data',
    '-e', 'JWT_SECRET=isolated-smoke-only-not-a-production-secret',
    '-e', 'BOOTSTRAP_ADMIN_USERNAME=smoke-admin',
    '-e', "BOOTSTRAP_ADMIN_PASSWORD=$([guid]::NewGuid().ToString('N'))",
    "jojhaa/sublinkx-rs-backend:$Version") | Out-Null
  DockerChecked @('run', '-d', '--name', $frontend, '--network', $network,
    '-p', '127.0.0.1::80', "jojhaa/sublinkx-rs-frontend:$Version") | Out-Null
  $port = (DockerChecked @('port', $frontend, '80/tcp')).Trim().Split(':')[-1]
  $base = "http://127.0.0.1:$port"
  $ready = $false
  for ($attempt = 0; $attempt -lt 60; $attempt++) {
    if ((DockerChecked @('inspect', $backend, '--format', '{{.State.Running}}')).Trim() -ne 'true') {
      throw 'Backend exited before readiness'
    }
    try {
      $response = Invoke-RestMethod "$base/api/v1/version" -TimeoutSec 3 -NoProxy
      if ($response.version -eq $Version) { $ready = $true; break }
    } catch { Start-Sleep -Seconds 1 }
  }
  if (-not $ready) { throw 'Backend readiness failed' }
  foreach ($path in @('/', '/healthz')) {
    if ((Invoke-WebRequest "$base$path" -TimeoutSec 5 -NoProxy).StatusCode -ne 200) { throw "Failed: $path" }
  }
  $unauthorized = Invoke-WebRequest "$base/api/v1/nodes" -SkipHttpErrorCheck -NoProxy
  if ($unauthorized.StatusCode -ne 401) { throw 'Authentication boundary failed' }
  foreach ($container in @($backend, $frontend)) {
    $restart = DockerChecked @('inspect', $container, '--format', '{{.RestartCount}}')
    if ([int]$restart -ne 0) { throw 'Unexpected container restart' }
  }
  Write-Output "PASS version=$Version home=200 health=200 anonymous_nodes=401 restarts=0"
} catch {
  & docker logs --tail 30 $backend
  & docker logs --tail 15 $frontend
  throw
} finally {
  & docker rm -f $frontend $backend 2>$null | Out-Null
  & docker network rm $network 2>$null | Out-Null
}
