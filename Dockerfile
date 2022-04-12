FROM rust:1.60-alpine as builder

WORKDIR /volume

RUN apk add --no-cache musl-dev=~1.2

COPY src/ src/
COPY Cargo.lock Cargo.toml ./

RUN cargo build --release

FROM alpine:3.15 as newuser

RUN echo "togglebot:x:1000:" > /tmp/group && \
    echo "togglebot:x:1000:1000::/dev/null:/sbin/nologin" > /tmp/passwd

FROM scratch

COPY --from=builder /volume/target/release/togglebot /bin/
COPY --from=newuser /tmp/group /tmp/passwd /etc/

USER togglebot

ENTRYPOINT ["/bin/togglebot"]
