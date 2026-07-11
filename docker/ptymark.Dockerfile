# syntax=docker/dockerfile:1.7

ARG NODE_IMAGE=node:24.18.0-bookworm
FROM ${NODE_IMAGE}

ARG RUST_VERSION=1.97.0
ARG MERMAID_CLI_VERSION=11.16.0
ARG TYPST_VERSION=0.15.0

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

ENV DEBIAN_FRONTEND=noninteractive \
    PUPPETEER_SKIP_DOWNLOAD=true \
    PUPPETEER_EXECUTABLE_PATH=/usr/bin/chromium \
    CARGO_HOME=/home/node/.cargo \
    RUSTUP_HOME=/home/node/.rustup \
    CARGO_TARGET_DIR=/home/node/.cache/ptymark-target \
    PATH=/home/node/.cargo/bin:${PATH}

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        bash \
        build-essential \
        ca-certificates \
        chromium \
        curl \
        fish \
        fontconfig \
        fonts-noto-cjk \
        fonts-noto-color-emoji \
        fonts-noto-core \
        git \
        jq \
        less \
        lua5.4 \
        make \
        pkg-config \
        python3 \
        python3-pip \
        rsync \
        shellcheck \
        zsh \
    && rm -rf /var/lib/apt/lists/*

RUN npm install --global "@mermaid-js/mermaid-cli@${MERMAID_CLI_VERSION}" \
    && npm cache clean --force

RUN mkdir -p \
        /workspace \
        /home/node/.cargo/registry \
        /home/node/.cargo/git \
        /home/node/.rustup \
        /home/node/.cache/ptymark-target \
    && chown -R node:node /workspace /home/node/.cargo /home/node/.rustup /home/node/.cache

USER node

RUN curl --proto '=https' --tlsv1.2 --fail --silent --show-error \
        https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain "${RUST_VERSION}" \
    && rustup component add clippy rustfmt --toolchain "${RUST_VERSION}" \
    && cargo install typst-cli --version "${TYPST_VERSION}" --locked

RUN rustc --version \
    && cargo --version \
    && node --version \
    && mmdc --version \
    && typst --version \
    && lua5.4 -v \
    && chromium --version

WORKDIR /workspace

CMD ["bash"]
