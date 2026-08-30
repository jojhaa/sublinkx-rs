param(
  [Parameter(Mandatory = $true)]
  [string] $Namespace,

  [string] $Registry = "docker.io",
  [string] $Tag = "latest",
  [string] $BackendImageName = "sublinkx-rs-backend",
  [string] $FrontendImageName = "sublinkx-rs-frontend",
  [string] $Platform = "linux/amd64",
  [string] $DockerRegistryMirror = "docker.m.daocloud.io",
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

if (-not $SkipBuild) {
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

if (-not $SkipPush) {
  Write-Host "Pushing backend..."
  docker push $backendRemote
  if ($LASTEXITCODE -ne 0) {
    throw "Backend image push failed with exit code $LASTEXITCODE"
  }

  Write-Host "Pushing frontend..."
  docker push $frontendRemote
  if ($LASTEXITCODE -ne 0) {
    throw "Frontend image push failed with exit code $LASTEXITCODE"
  }
}

Write-Host ""
Write-Host $(if ($SkipPush) { 'Built images:' } else { 'Pushed images:' })
Write-Host "  $backendRemote"
Write-Host "  $frontendRemote"
