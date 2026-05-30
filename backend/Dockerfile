ARG DOCKER_REGISTRY=docker.io

FROM ${DOCKER_REGISTRY}/library/rust:1-bookworm AS builder

WORKDIR /workspace/backend

RUN mkdir -p /usr/local/cargo \
    && printf '[source.crates-io]\nreplace-with = "rsproxy-sparse"\n\n[source.rsproxy-sparse]\nregistry = "sparse+https://rsproxy.cn/index/"\n\n[net]\ngit-fetch-with-cli = true\n' > /usr/local/cargo/config.toml

ENV CARGO_HTTP_MULTIPLEXING=false

COPY backend/Cargo.toml backend/Cargo.lock ./
COPY backend/migrations ./migrations
COPY backend/src ./src
COPY docs ../docs

RUN cargo build --release

FROM ${DOCKER_REGISTRY}/library/debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /workspace/backend/target/release/sublinkx-rs-backend /usr/local/bin/sublinkx-rs-backend

RUN mkdir -p /app/data /app/mihomo

ENV APP_ENV=production
ENV APP_PORT=8080
ENV DATABASE_URL=sqlite:///app/data/app.db

EXPOSE 8080

CMD ["sublinkx-rs-backend"]
