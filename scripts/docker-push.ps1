param(
  [Parameter(Mandatory = $true)]
  [string] $Namespace,

  [string] $Registry = "docker.io",
  [string] $Tag = "latest",
  [string] $BackendImageName = "sublinkx-rs-backend",
  [string] $FrontendImageName = "sublinkx-rs-frontend",
  [string] $Platform = "linux/amd64",
  [string] $DockerRegistryMirror = "docker.m.daocloud.io",
  [switch] $BackendOnly,
  [switch] $FrontendOnly,
  [switch] $SkipBuild,
  [switch] $SkipPush
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$backendRemote = "$Registry/$Namespace/$BackendImageName`:$Tag"
$frontendRemote = "$Registry/$Namespace/$FrontendImageName`:$Tag"

if ($SkipBuild -and $SkipPush) {
  throw 'SkipBuild and SkipPush cannot be used together.'
}
if ($BackendOnly -and $FrontendOnly) {
  throw 'BackendOnly and FrontendOnly cannot be used together.'
}

$processBackend = -not $FrontendOnly
$processFrontend = -not $BackendOnly

if (-not $SkipBuild) {
  if ($processBackend) {
    Write-Host "Building backend locally: $backendRemote"
    docker buildx build `
      --platform $Platform `
      --load `
      --provenance=false `
      --build-arg "DOCKER_REGISTRY=$DockerRegistryMirror" `
      --file backend/Dockerfile `
      --tag $backendRemote `
      .
    if ($LASTEXITCODE -ne 0) {
      throw "Backend image build failed with exit code $LASTEXITCODE"
    }
  }

  if ($processFrontend) {
    Write-Host "Building frontend locally: $frontendRemote"
    docker buildx build `
      --platform $Platform `
      --load `
      --provenance=false `
      --build-arg "DOCKER_REGISTRY=$DockerRegistryMirror" `
      --file frontend/Dockerfile `
      --tag $frontendRemote `
      frontend
    if ($LASTEXITCODE -ne 0) {
      throw "Frontend image build failed with exit code $LASTEXITCODE"
    }
  }
}

if (-not $SkipPush) {
  if ($processBackend) {
    Write-Host "Pushing backend..."
    docker push $backendRemote
    if ($LASTEXITCODE -ne 0) {
      throw "Backend image push failed with exit code $LASTEXITCODE"
    }
  }

  if ($processFrontend) {
    Write-Host "Pushing frontend..."
    docker push $frontendRemote
    if ($LASTEXITCODE -ne 0) {
      throw "Frontend image push failed with exit code $LASTEXITCODE"
    }
  }
}

Write-Host ""
Write-Host $(if ($SkipPush) { 'Built images:' } else { 'Pushed images:' })
if ($processBackend) {
  Write-Host "  $backendRemote"
}
if ($processFrontend) {
  Write-Host "  $frontendRemote"
}
