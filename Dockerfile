# syntax=docker/dockerfile:1.7
#
# rbara — PDF prepress CLI / TUI
#
# Multi-stage build:
#   1. builder  — rust:1.85-bookworm, compiles rbara + downloads pdfium
#   2. runtime  — debian:bookworm-slim, ~60–80 MB, ships rbara + libpdfium.so
#
# Build:
#   docker build -t rbara:0.1.3 -t rbara:latest .
#   # Pin a different pdfium build (default 7776 = PDFium 148.0.7776.0):
#   docker build --build-arg PDFIUM_CHROMIUM=7811 -t rbara:dev .
#
# Run (CLI — recommended in containers):
#   docker run --rm -v "$PWD:/work" rbara trim /work/in.pdf /work/out.pdf
#
# Run (TUI — needs a real terminal):
#   docker run --rm -it -v "$PWD:/work" rbara
#
# License: rbara is GPL-3.0-only (binary). The bundled pdfium binary ships
# under its own Apache-2.0 / BSD-3-Clause license (see /opt/rbara/share/).

ARG RUST_VERSION=1.88
ARG DEBIAN_CODENAME=bookworm
ARG PDFIUM_CHROMIUM=7776

# ---------------------------------------------------------------------------
# Stage 1: builder
# ---------------------------------------------------------------------------
FROM rust:${RUST_VERSION}-${DEBIAN_CODENAME} AS builder

ARG PDFIUM_CHROMIUM
ENV DEBIAN_FRONTEND=noninteractive

# Build deps:
#   liblcms2-dev  -> rustybara `color` feature (lcms2 crate links against it)
#   pkg-config    -> lcms2-sys discovers the lib via pkg-config
#   ca-certificates + curl -> fetch pdfium release tarball
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
        liblcms2-dev \
        pkg-config \
        ca-certificates \
        curl \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /src

# Copy the whole workspace (Cargo.lock is at the workspace root).
# Order chosen so source changes don't bust the apt layer above.
COPY Cargo.toml Cargo.lock* ./
COPY rustybara/     rustybara/
COPY rbara/         rbara/
COPY rbv/           rbv/
# Cargo needs every workspace member's Cargo.toml to resolve the workspace,
# even for members that aren't built in this image.
RUN mkdir -p rbara-gui
COPY rustybara-icc/           rustybara-icc/
COPY rbara-gui/Cargo.toml     rbara-gui/Cargo.toml
RUN mkdir -p rustybara-wasm
COPY rustybara-wasm/Cargo.toml rustybara-wasm/Cargo.toml
COPY README.md LICENSE-LGPL-3.0 LICENSE-GPL-3.0 ./

# Build only the `rbara` binary (skip rbv stub).
RUN cargo build --release -p rbara \
 && strip target/release/rbara

# Fetch pdfium libpdfium.so (Linux x64) and stage it for the runtime layer.
# Pinned via PDFIUM_CHROMIUM to match the Linux installer (chromium/7776 default).
RUN set -eux; \
    mkdir -p /pdfium-stage; \
    curl -fL --retry 3 --retry-delay 2 \
        -o /tmp/pdfium.tgz \
        "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F${PDFIUM_CHROMIUM}/pdfium-linux-x64.tgz"; \
    mkdir -p /tmp/pdfium && tar -xzf /tmp/pdfium.tgz -C /tmp/pdfium; \
    cp /tmp/pdfium/lib/libpdfium.so /pdfium-stage/libpdfium.so; \
    cp /tmp/pdfium/LICENSE          /pdfium-stage/LICENSE-pdfium 2>/dev/null \
        || cp -r /tmp/pdfium/LICENSES /pdfium-stage/LICENSES-pdfium 2>/dev/null \
        || true; \
    rm -rf /tmp/pdfium /tmp/pdfium.tgz

# ---------------------------------------------------------------------------
# Stage 2: runtime
# ---------------------------------------------------------------------------
FROM debian:${DEBIAN_CODENAME}-slim AS runtime

ARG PDFIUM_CHROMIUM
ENV DEBIAN_FRONTEND=noninteractive

# Runtime deps:
#   liblcms2-2     -> required by rbara (color feature)
#   ca-certificates-> for any future HTTPS work; tiny
#   libgcc-s1, libstdc++6, zlib1g -> already pulled in by libc6/base, listed
#                                    explicitly so layer caches survive minor
#                                    base-image churn
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
        liblcms2-2 \
        ca-certificates \
        libgcc-s1 \
        libstdc++6 \
        zlib1g \
 && rm -rf /var/lib/apt/lists/* \
 && apt-get clean

# Non-root user (uid 1000 to match common host uids for bind-mount writes).
RUN groupadd --system --gid 1000 rbara \
 && useradd  --system --uid 1000 --gid rbara --create-home --home-dir /home/rbara --shell /bin/bash rbara

# Layout mirrors the Linux tarball installer:
#   /opt/rbara/bin/rbara-bin          (the binary)
#   /opt/rbara/lib/libpdfium.so       (bundled pdfium)
#   /opt/rbara/share/                 (licenses)
#   /usr/local/bin/rbara              (tiny exec shim → on PATH)
RUN mkdir -p /opt/rbara/bin /opt/rbara/lib /opt/rbara/share /work \
 && chown -R rbara:rbara /work

COPY --from=builder /src/target/release/rbara         /opt/rbara/bin/rbara-bin
COPY --from=builder /pdfium-stage/libpdfium.so        /opt/rbara/lib/libpdfium.so
COPY --from=builder /src/LICENSE-GPL-3.0              /opt/rbara/share/LICENSE.txt
COPY --from=builder /pdfium-stage/                    /opt/rbara/share/pdfium/

# Ensure pdfium is found by rbara's runtime dlopen.
ENV LD_LIBRARY_PATH=/opt/rbara/lib

# Tiny shim so users (and the ENTRYPOINT) just call `rbara`.
RUN printf '#!/bin/sh\nexec /opt/rbara/bin/rbara-bin "$@"\n' > /usr/local/bin/rbara \
 && chmod 0755 /usr/local/bin/rbara /opt/rbara/bin/rbara-bin /opt/rbara/lib/libpdfium.so

# Image labels (OCI standard).
LABEL org.opencontainers.image.title="rbara" \
      org.opencontainers.image.description="Prepress-focused PDF CLI/TUI (trim, bleed, rasterize, color remap)" \
      org.opencontainers.image.licenses="GPL-3.0-only" \
      org.opencontainers.image.source="https://github.com/Addy-A/rustybara" \
      org.opencontainers.image.documentation="https://github.com/Addy-A/rustybara#readme"

USER rbara
WORKDIR /work

ENTRYPOINT ["/usr/local/bin/rbara"]
CMD ["--help"]
