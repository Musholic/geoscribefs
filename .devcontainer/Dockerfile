FROM mcr.microsoft.com/devcontainers/rust:latest

# Enable contrib repository for zfsutils
RUN sed -i 's/Components: main/Components: main contrib/' /etc/apt/sources.list.d/debian.sources

# Install required tools
RUN apt-get update && apt-get install -y --no-install-recommends \
    protobuf-compiler \
    btrfs-progs \
    zfsutils-linux \
    && rm -rf /var/lib/apt/lists/*

# Install cargo-nextest via cargo-binstall
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash

RUN cargo binstall cargo-nextest --secure
