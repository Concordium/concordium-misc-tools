ARG build_image
ARG base_image
FROM ${build_image} AS build

WORKDIR /build
COPY chain-prometheus-exporter chain-prometheus-exporter
COPY deps/concordium-rust-sdk deps/concordium-rust-sdk
RUN cargo build --locked --manifest-path chain-prometheus-exporter/Cargo.toml --release

FROM ${base_image}
RUN apt-get update && \
    apt-get -y install \
      ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /build/chain-prometheus-exporter/target/release/chain-prometheus-exporter /usr/local/bin/

