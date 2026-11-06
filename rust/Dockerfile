FROM rust:1.99-slim-bookworm AS builder

WORKDIR /build

# Cache dependency compilation separately from the application sources.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && printf 'fn main() {}\n' > src/main.rs \
    && cargo build --release --locked \
    && rm -rf src

COPY schema_revisions.txt ./
COPY src ./src
COPY assets ./assets
RUN touch src/main.rs && cargo build --release --locked

FROM debian:bookworm-slim AS runtime

ARG SOURCE_COMMIT=unknown
ENV COMMIT_SHA=${SOURCE_COMMIT}

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system app \
    && useradd --system --gid app --home-dir /app --no-create-home app

WORKDIR /app
COPY --from=builder /build/target/release/past-exam-service /usr/local/bin/past-exam-service

USER app
EXPOSE 8080

HEALTHCHECK --interval=15s --timeout=5s --start-period=20s --retries=5 \
    CMD ["/usr/local/bin/past-exam-service", "healthcheck"]

ENTRYPOINT ["/usr/local/bin/past-exam-service", "serve"]
