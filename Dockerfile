# syntax=docker/dockerfile:1
# Release builds may override these with reviewed tag@sha256:digest references.
ARG RUST_IMAGE=rust:1.99.1-alpine3.24
ARG NODE_IMAGE=node:24.22.0-alpine3.24
ARG SWS_IMAGE=ghcr.io/static-web-server/static-web-server:2.44.1-alpine

FROM ${RUST_IMAGE} AS wasm
WORKDIR /app
# Native build tools are needed by wasm-bindgen-cli and its dependencies.
RUN apk add --no-cache build-base \
    && rustup target add wasm32-unknown-unknown
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
# The CLI must match the wasm-bindgen library version in Cargo.lock.
RUN set -eu; version="$(awk '/name = "wasm-bindgen"/ {getline; gsub(/"/, "", $3); print $3; exit}' Cargo.lock)"; \
    test -n "$version"; \
    cargo install wasm-bindgen-cli --version "$version" --locked
RUN cargo test --locked -p ibasso-protocol -p ibasso-wasm \
    && cargo build --locked --release -p ibasso-wasm --target wasm32-unknown-unknown \
    && wasm-bindgen --target web --out-dir /pkg target/wasm32-unknown-unknown/release/ibasso_wasm.wasm

FROM ${NODE_IMAGE} AS web
ARG PNPM_VERSION=11.25.0
RUN npm install --global pnpm@${PNPM_VERSION}
WORKDIR /app/web
COPY web/package.json web/pnpm-lock.yaml ./
# Install Linux/musl optional dependencies; never copy host node_modules.
RUN pnpm install --frozen-lockfile --ignore-scripts
COPY web/ ./
COPY --from=wasm /pkg ./pkg
# WASM was already compiled in the Rust stage.
RUN pnpm exec tsc --noEmit && pnpm exec vite build

# Artifact-only stage: no server, ports, entrypoint, or OS runtime.
# docker build --pull --target export --output type=local,dest=./dist .
FROM scratch AS export
COPY --from=web /app/web/dist/ /

# Default runnable HTTP image; keep export available as an explicit target.
FROM ${SWS_IMAGE} AS runtime
COPY --from=web /app/web/dist/ /public/
COPY web/deploy/sws.toml /etc/sws.toml
USER 65532:65532
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 CMD wget -q -O /dev/null http://127.0.0.1:8080/health || exit 1
ENTRYPOINT ["static-web-server"]
CMD ["--config-file", "/etc/sws.toml"]
