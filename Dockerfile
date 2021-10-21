FROM rust:1.56 as builder

WORKDIR /volume

RUN apt-get update && \
    apt-get install -y --no-install-recommends musl-tools=1.2.2-1 && \
    rustup target add x86_64-unknown-linux-musl

COPY src/ src/
COPY Cargo.lock Cargo.toml ./

RUN cargo build --release --target x86_64-unknown-linux-musl && \
    strip --strip-all target/x86_64-unknown-linux-musl/release/togglebot

FROM alpine:3.14 as newuser

RUN echo "togglebot:x:1000:" > /tmp/group && \
    echo "togglebot:x:1000:1000::/dev/null:/sbin/nologin" > /tmp/passwd

FROM scratch

COPY --from=builder /volume/target/x86_64-unknown-linux-musl/release/togglebot /bin/
COPY --from=newuser /tmp/group /tmp/passwd /etc/

STOPSIGNAL SIGINT
USER togglebot

ENTRYPOINT ["/bin/togglebot"]
