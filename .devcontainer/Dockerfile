FROM mcr.microsoft.com/devcontainers/rust:latest

# Install required tools
RUN apt-get update && apt-get install -y --no-install-recommends \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*
