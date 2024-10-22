#!/bin/bash

set -ex

# Cd to project root dir
cd "$(dirname "${BASH_SOURCE[0]}")/.."

architectures=("amd64" "arm/v7" "aarch64")

# Check if VERSION_TAG is set, if not, get it from Cargo.toml
if [ -z "$VERSION_TAG" ]; then
    VERSION_TAG="$(cargo metadata --no-deps --format-version 1 | jq -r .packages[0].version)"
fi

# Determine registry part of the image tag
if [ -n "$REGISTRY" ]; then
    registry_tag="$REGISTRY/"
else
    registry_tag=""
fi

# Loop through architectures and build images
for i in "${!architectures[@]}"; do
    podman build \
        --pull \
        --platform "linux/${architectures[$i]}" \
        -t "${registry_tag}light-operator:$VERSION_TAG-${architectures[$i]//\//_}" .
done

# Create manifest list
podman manifest create --amend "${registry_tag}light-operator:$VERSION_TAG" \
    "containers-storage:${registry_tag}light-operator:$VERSION_TAG-amd64" \
    "containers-storage:${registry_tag}light-operator:$VERSION_TAG-arm_v7" \
    "containers-storage:${registry_tag}light-operator:$VERSION_TAG-aarch64"

echo "Created manifest \"${registry_tag}light-operator:$VERSION_TAG\""