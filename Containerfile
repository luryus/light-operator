FROM --platform=$BUILDPLATFORM docker.io/library/rust:1-slim-bookworm AS chef
WORKDIR /app
RUN apt-get update && apt-get install -y gcc-aarch64-linux-gnu libc6-dev-arm64-cross gcc-arm-linux-gnueabihf libc6-dev-armhf-cross
RUN --mount=type=cache,destination=/usr/local/cargo/registry,id=cargo-reg-cache \
    cargo install cargo-chef --locked
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=/usr/bin/aarch64-linux-gnu-gcc
ENV CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=/usr/bin/arm-linux-gnueabihf-gcc
ARG TARGETPLATFORM
RUN case "$TARGETPLATFORM" in \
    linux/arm64) RUSTTARGET=aarch64-unknown-linux-gnu ;; \
    linux/amd64) RUSTTARGET=x86_64-unknown-linux-gnu ;; \
    linux/arm/v7) RUSTTARGET=armv7-unknown-linux-gnueabihf ;; \
    *) echo >&2 "Unsupported architecture $TARGETPLATFORM"; exit 1 ;; \
    esac ; \
    echo "$RUSTTARGET" > rusttarget

FROM --platform=$BUILDPLATFORM chef AS planner
COPY . .
RUN --mount=type=cache,destination=/usr/local/cargo/registry,id=cargo-reg-cache \
    cargo chef prepare --recipe-path recipe.json

FROM --platform=$BUILDPLATFORM chef AS builder
RUN rustup target add "$(cat rusttarget)"
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,destination=/usr/local/cargo/registry,id=cargo-reg-cache \
    cargo chef cook --target="$(cat rusttarget)" --release --recipe-path recipe.json
COPY . .
RUN --mount=type=cache,destination=/usr/local/cargo/registry,id=cargo-reg-cache \
    cargo build --target="$(cat rusttarget)" --release --locked --bin controller ; \
    mkdir -p out; cp "target/$(cat rusttarget)/release/controller" out/

FROM docker.io/library/debian:bookworm-slim AS runtime
WORKDIR /app
COPY --from=builder "/app/out/controller" /usr/local/bin
COPY --from=builder /app/config.yaml .
ENTRYPOINT ["/usr/local/bin/controller"]
