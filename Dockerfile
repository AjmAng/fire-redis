# phase 1: builder
FROM rust:1-alpine3.20 AS builder


RUN apk add --no-cache musl-dev pkgconfig

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY version ./version
RUN cargo build --release --target x86_64-unknown-linux-musl

# phase 2: runtime
FROM alpine:3.20

# add basic dependencies and create non-root user
RUN apk add --no-cache ca-certificates && \
    addgroup -g 1000 redis && \
    adduser -D -s /bin/sh -u 1000 -G redis redis

# copy the compiled server / cli from the builder stage
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/redis-server /usr/local/bin/redis
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/redis-cli /usr/local/bin/redis-cli

# set up a volume for data persistence and set the working directory & change ownership to the non-root user
VOLUME ["/data"]
WORKDIR /data
# phase 2: runtime
FROM alpine:3.20

RUN apk add --no-cache ca-certificates && \
    addgroup -g 1000 redis && \
    adduser -D -s /bin/sh -u 1000 -G redis redis && \
    mkdir -p /data && \
    chown -R redis:redis /data

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/redis-server /usr/local/bin/redis
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/redis-cli /usr/local/bin/redis-cli

VOLUME ["/data"]
WORKDIR /data

ENV REDIS_BIND=0.0.0.0
ENV REDIS_PORT=6379

USER redis
EXPOSE 6379
ENTRYPOINT ["redis"]

# switch to the non-root user
USER redis

# expose the default Redis port
EXPOSE 6379

ENV REDIS_BIND=0.0.0.0
ENV REDIS_PORT=6379

# health check to ensure the application is running properly
# HEALTHCHECK --interval=5s --timeout=3s --start-period=5s --retries=3 \
#    CMD redis-cli -p 6379 ping || exit 1

ENTRYPOINT ["redis"]
