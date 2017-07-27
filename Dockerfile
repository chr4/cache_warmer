FROM ubuntu:16.04

RUN apt-get update \
 && apt-get install -y --no-install-recommends --no-install-suggests \
    ca-certificates curl libssl-dev build-essential pkg-config \
 && rm -rf /var/lib/apt/lists/*

RUN curl https://sh.rustup.rs -sSf | \
    sh -s -- --default-toolchain stable -y

ENV PATH=/root/.cargo/bin:$PATH
ENV CARGO_HOME=/app/.cargo
WORKDIR /app

CMD ["cargo", "build", "--release"]
