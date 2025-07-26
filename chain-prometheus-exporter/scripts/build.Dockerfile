ARG build_image=rust:1.76-bookworm
ARG base_image=debian:bookworm-slim
FROM ${build_image} AS build

WORKDIR /build
COPY chain-prometheus-exporter chain-prometheus-exporter
COPY deps/concordium-rust-sdk deps/concordium-rust-sdk
RUN cargo build --locked -p chain-prometheus-exporter --release

FROM ${base_image}
RUN apt-get update && \
    apt-get -y install \
      ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /build/target/release/chain-prometheus-exporter /usr/local/bin/

