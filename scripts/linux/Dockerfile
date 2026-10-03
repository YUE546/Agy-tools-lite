FROM ubuntu:22.04
ARG RUST_VERSION=1.98.1
ARG NODE_VERSION=22.23.3
ENV DEBIAN_FRONTEND=noninteractive RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl xz-utils build-essential git pkg-config cmake clang libclang-dev perl \
    libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev \
    libssl-dev patchelf && rm -rf /var/lib/apt/lists/*
RUN curl -fsSL https://sh.rustup.rs -o /tmp/rustup-init.sh && \
    sh /tmp/rustup-init.sh -y --profile minimal --default-toolchain "$RUST_VERSION" --no-modify-path && \
    chmod -R a+rX /opt/rustup /opt/cargo && rm /tmp/rustup-init.sh
RUN case "$(uname -m)" in x86_64) task_arch=x64 ;; aarch64) task_arch=arm64 ;; *) exit 1 ;; esac && \
    cd /tmp && task_archive="node-v${NODE_VERSION}-linux-${task_arch}.tar.xz" && \
    curl -fsSL "https://nodejs.org/dist/v${NODE_VERSION}/${task_archive}" -o "$task_archive" && \
    curl -fsSL "https://nodejs.org/dist/v${NODE_VERSION}/SHASUMS256.txt" -o SHASUMS256.txt && \
    awk -v file="$task_archive" '$2 == file' SHASUMS256.txt | sha256sum -c - && \
    mkdir -p /opt/node && tar -xJf "$task_archive" -C /opt/node --strip-components=1 && \
    rm "$task_archive" SHASUMS256.txt
ENV PATH=/opt/node/bin:/opt/cargo/bin:$PATH
WORKDIR /work
