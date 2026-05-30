param(
  [Parameter(Mandatory = $true)]
  [string] $Namespace,

  [string] $Registry = "docker.io",
  [string] $Tag = "latest",
  [string] $BackendImageName = "sublinkx-rs-backend",
  [string] $FrontendImageName = "sublinkx-rs-frontend"
)

$ErrorActionPreference = "Stop"

$backendRemote = "$Registry/$Namespace/$BackendImageName`:$Tag"
$frontendRemote = "$Registry/$Namespace/$FrontendImageName`:$Tag"

Write-Host "Building local images..."
docker compose build

Write-Host "Tagging backend: $backendRemote"
docker tag "sublinkx-rs-backend:local" $backendRemote

Write-Host "Tagging frontend: $frontendRemote"
docker tag "sublinkx-rs-frontend:local" $frontendRemote

Write-Host "Pushing backend..."
docker push $backendRemote

Write-Host "Pushing frontend..."
docker push $frontendRemote

Write-Host ""
Write-Host "Pushed images:"
Write-Host "  $backendRemote"
Write-Host "  $frontendRemote"
