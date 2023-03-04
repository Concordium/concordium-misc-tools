ARG build_image
ARG base_image
FROM ${build_image} AS build

WORKDIR /build
COPY network-dashboard-proxy network-dashboard-proxy
COPY deps/concordium-rust-sdk deps/concordium-rust-sdk
RUN cargo build --locked --manifest-path network-dashboard-proxy/Cargo.toml --release

FROM ${base_image}
RUN apt-get update && \
    apt-get -y install \
      ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /build/network-dashboard-proxy/target/release/network-dashboard-proxy /usr/local/bin/

