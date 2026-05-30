#!/usr/bin/env sh
set -eu

NAMESPACE="${1:?usage: ./scripts/docker-push.sh <namespace> [tag] [registry]}"
TAG="${2:-latest}"
REGISTRY="${3:-docker.io}"
BACKEND_IMAGE_NAME="${BACKEND_IMAGE_NAME:-sublinkx-rs-backend}"
FRONTEND_IMAGE_NAME="${FRONTEND_IMAGE_NAME:-sublinkx-rs-frontend}"

BACKEND_REMOTE="$REGISTRY/$NAMESPACE/$BACKEND_IMAGE_NAME:$TAG"
FRONTEND_REMOTE="$REGISTRY/$NAMESPACE/$FRONTEND_IMAGE_NAME:$TAG"

echo "Building local images..."
docker compose build

echo "Tagging backend: $BACKEND_REMOTE"
docker tag "sublinkx-rs-backend:local" "$BACKEND_REMOTE"

echo "Tagging frontend: $FRONTEND_REMOTE"
docker tag "sublinkx-rs-frontend:local" "$FRONTEND_REMOTE"

echo "Pushing backend..."
docker push "$BACKEND_REMOTE"

echo "Pushing frontend..."
docker push "$FRONTEND_REMOTE"

echo "Pushed images:"
echo "  $BACKEND_REMOTE"
echo "  $FRONTEND_REMOTE"
