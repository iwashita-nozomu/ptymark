# syntax=docker/dockerfile:1.7
# @dependency-start
# contract environment
# responsibility Builds the canonical ptymark Rust, WezTerm-plugin, and selected-renderer validation environment.
# upstream environment ./ptymark-versions.env pins the compiler and Node base image.
# upstream environment ../renderers/package-lock.json pins the renderer dependency graph.
# downstream workflow ../.github/workflows/ptymark-ci.yml runs the canonical checks.
# @dependency-end
ARG NODE_IMAGE=node:24.18.0-bookworm
FROM ${NODE_IMAGE}

ARG RUST_VERSION=1.97.0

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

ENV DEBIAN_FRONTEND=noninteractive \
    PUPPETEER_SKIP_DOWNLOAD=true \
    PUPPETEER_EXECUTABLE_PATH=/usr/bin/chromium \
    PTYMARK_RENDERER_ROOT=/opt/ptymark-renderers \
    CARGO_HOME=/home/node/.cargo \
    RUSTUP_HOME=/home/node/.rustup \
    CARGO_TARGET_DIR=/home/node/.cache/ptymark-target \
    PATH=/opt/ptymark-renderers/node_modules/.bin:/home/node/.cargo/bin:${PATH}

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        bash \
        build-essential \
        ca-certificates \
        chromium \
        curl \
        git \
        lua5.4 \
        make \
        pkg-config \
        shellcheck \
    && rm -rf /var/lib/apt/lists/*

RUN mkdir -p \
        /workspace \
        /opt/ptymark-renderers \
        /home/node/.cargo \
        /home/node/.rustup \
        /home/node/.cache/ptymark-target \
    && chown -R node:node \
        /workspace \
        /opt/ptymark-renderers \
        /home/node/.cargo \
        /home/node/.rustup \
        /home/node/.cache

COPY --chown=node:node \
    renderers/package.json \
    renderers/package-lock.json \
    /opt/ptymark-renderers/
COPY --chown=node:node renderers/check.mjs /opt/ptymark-renderers/check.mjs

USER node

RUN npm ci \
        --prefix /opt/ptymark-renderers \
        --omit=dev \
        --ignore-scripts \
    && npm cache clean --force

RUN curl --proto '=https' --tlsv1.2 --fail --silent --show-error \
        https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain "${RUST_VERSION}" \
    && rustup component add clippy rustfmt --toolchain "${RUST_VERSION}"

RUN rustc --version \
    && cargo --version \
    && node --version \
    && mmdc --version \
    && lua5.4 -v \
    && chromium --version

WORKDIR /workspace
CMD ["bash"]
