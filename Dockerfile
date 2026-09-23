# syntax=docker/dockerfile:1

# Toolchain only: rebuilt when this block changes, not when the source does.
FROM rust:1-bookworm AS toolchain

# Install system dependencies (clang required by cargo-xwin)
RUN apt-get update && apt-get install -y \
    build-essential \
    curl \
    wget \
    file \
    pkg-config \
    libssl-dev \
    clang \
    llvm \
    && rm -rf /var/lib/apt/lists/*

# Install Node.js 24 and pnpm
RUN curl -fsSL https://deb.nodesource.com/setup_24.x | bash - && \
    apt-get install -y nodejs && \
    npm install -g pnpm@9

# Add Windows MSVC target and install cargo-xwin
RUN rustup target add x86_64-pc-windows-msvc && \
    cargo install cargo-xwin --locked

# Downloaded MSVC CRT / Windows SDK; kept in a cache mount below.
ENV XWIN_CACHE_DIR=/root/.cache/cargo-xwin

WORKDIR /app

FROM toolchain AS builder

# Frontend dependencies first, so source edits don't trigger a reinstall.
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml .npmrc ./
RUN --mount=type=cache,id=pnpm-store,target=/pnpm/store \
    pnpm install --frozen-lockfile --store-dir /pnpm/store

COPY . .
RUN pnpm build

# Cross-compile Windows .exe
# `custom-protocol` embeds dist/ into the binary. Without it tauri compiles in
# dev mode and the window points at devUrl (localhost), showing a blank UI.
# Crate downloads, the MSVC SDK and target/ live in cache mounts, so only
# changed crates are recompiled. The .exe is copied out because cache mounts
# are not part of the image.
# PROFILE=fast-release (see src-tauri/Cargo.toml) trades a slightly larger,
# slower binary for much quicker local builds.
ARG PROFILE=release
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=cargo-xwin,target=/root/.cache/cargo-xwin \
    --mount=type=cache,id=pdfsan-target,target=/app/src-tauri/target \
    cd src-tauri && \
    cargo xwin build --profile "$PROFILE" --features custom-protocol --target x86_64-pc-windows-msvc && \
    mkdir -p /out && \
    cp "target/x86_64-pc-windows-msvc/$PROFILE/pdf-sanitizer.exe" /out/

# Export stage: `docker build --target export --output . .` writes the .exe
# straight to the current directory.
FROM scratch AS export
COPY --from=builder /out/pdf-sanitizer.exe /

# Final stage — minimal image containing only the .exe for extraction
FROM debian:bookworm-slim
COPY --from=builder /out/pdf-sanitizer.exe /pdf-sanitizer.exe

# To extract the binary:
#   docker build --target export --output . .
# or:
#   docker build -t pdf-sanitizer-builder .
#   docker create --name extract pdf-sanitizer-builder
#   docker cp extract:/pdf-sanitizer.exe ./pdf-sanitizer.exe
#   docker rm extract
