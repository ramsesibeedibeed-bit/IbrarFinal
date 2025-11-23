FROM ubuntu:24.04

# Allow proxy values to be passed at build time (some networks require this)
ARG HTTP_PROXY
ARG HTTPS_PROXY
ENV HTTP_PROXY=${HTTP_PROXY}
ENV HTTPS_PROXY=${HTTPS_PROXY}

ENV DEBIAN_FRONTEND=noninteractive
ENV PATH="/root/.cargo/bin:/root/.local/share/solana/install/active_release/bin:${PATH}"

RUN apt-get update \
  && apt-get install -y --no-install-recommends \
    build-essential clang llvm libssl-dev pkg-config libclang-dev make python3 ca-certificates curl git bzip2 pkgconf ca-certificates \ 
  && rm -rf /var/lib/apt/lists/*

# Install rustup and stable toolchain
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
  && . /root/.cargo/env \
  && rustup default stable

# Install Solana CLI v1.18.26
RUN curl -sSfL https://release.solana.com/v1.18.26/install | sh -s

# Install anchor-cli v0.31.1 (matches Anchor.toml)
RUN /bin/bash -lc ". /root/.cargo/env && cargo install --locked --git https://github.com/coral-xyz/anchor --tag v0.31.1 anchor-cli || true"

# Install cargo-build-sbf (needed by Anchor to build SBF programs)
# Install cargo-build-sbf from the Solana Labs repo (provides `cargo build-sbf`)
# Use the git CLI for fetching to avoid cargo's internal fetch auth behavior
# Configure git to use proxy and avoid credential helpers during build
RUN /bin/bash -lc \
  ". /root/.cargo/env && \
  git config --global --unset credential.helper || true && \
  if [ -n \"$HTTP_PROXY\" ]; then git config --global http.proxy $HTTP_PROXY || true; fi && \
  if [ -n \"$HTTPS_PROXY\" ]; then git config --global https.proxy $HTTPS_PROXY || true; fi && \
  export CARGO_NET_GIT_FETCH_WITH_CLI=true && export GIT_TERMINAL_PROMPT=0 && \
  cargo install --locked --git https://github.com/solana-labs/cargo-build-sbf --tag v1.18.26 cargo-build-sbf || true"

WORKDIR /workspace

# Default entrypoint: run shell. We'll pass build commands via docker run.
ENTRYPOINT ["/bin/bash","-lc"]
