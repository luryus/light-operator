# Release Checklist

### Preparations
- Update version in Cargo.toml
- Update version numbers (`version` and `appVersion`) in charts/light-operator/Chart.yaml
- Run `cargo update`
- Commit changes

### Container images
- Run `REGISTRY="ghcr.io/luryus" scripts/build_container_images.sh`
- Run `podman manifest push ghcr.io/luryus/light-operator:0.2.0` (change version to the correct one)

### Chart
- Under `charts/`, run `helm package light-operator`
- Run `helm push light-operator-0.2.0.tgz oci://ghcr.io/luryus/charts` (change version to the correct one)

### Finally
- Tag commit
- Create release if you feel like it