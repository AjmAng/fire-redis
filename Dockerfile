# phase 1: builder
FROM rust:1-alpine3.20 AS builder


RUN apk add --no-cache musl-dev pkgconfig

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY benches ./benches
COPY version ./version
RUN cargo build --release --target x86_64-unknown-linux-musl

# phase 2: runtime
FROM alpine:3.20

RUN apk add --no-cache ca-certificates && \
    addgroup -g 1000 redis && \
    adduser -D -s /bin/sh -u 1000 -G redis redis && \
    mkdir -p /data && \
    chown -R redis:redis /data

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/redis-server /usr/local/bin/redis
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/redis-cli /usr/local/bin/redis-cli
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/redis-benchmark /usr/local/bin/redis-benchmark

VOLUME ["/data"]
WORKDIR /data

ENV REDIS_BIND=0.0.0.0
ENV REDIS_PORT=6379

USER redis
EXPOSE 6379
ENTRYPOINT ["redis"]

