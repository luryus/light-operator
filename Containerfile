FROM docker.io/library/rust:1-alpine3.18 AS chef
WORKDIR /app
RUN apk add --no-cache musl-dev
RUN --mount=type=cache,destination=/usr/local/cargo/registry,id=cargo-reg-cache \
    cargo install cargo-chef --locked

FROM chef AS planner
COPY . .
RUN --mount=type=cache,destination=/usr/local/cargo/registry,id=cargo-reg-cache \
    cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
ARG TARGETPLATFORM
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,destination=/usr/local/cargo/registry,id=cargo-reg-cache \
    cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN --mount=type=cache,destination=/usr/local/cargo/registry,id=cargo-reg-cache \
    cargo build --release --locked --bin controller

FROM docker.io/library/alpine:3.18 AS runtime
WORKDIR /app
COPY --from=builder /app/target/release/controller /usr/local/bin
COPY --from=builder /app/config.yaml .
CMD "/usr/local/bin/controller"