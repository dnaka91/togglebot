# syntax = docker/dockerfile:1.2
FROM clux/muslrust:stable as builder

WORKDIR /volume

COPY src/ src/
COPY Cargo.lock Cargo.toml ./

RUN --mount=type=cache,target=/root/.cargo/git \
    --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/volume/target \
    cargo install --locked --path .

RUN strip /root/.cargo/bin/togglebot

FROM alpine:3.14.0

WORKDIR /data

COPY --from=builder /root/.cargo/bin/togglebot /app/

STOPSIGNAL SIGINT

ENTRYPOINT ["/app/togglebot"]
