ARG build_image=rust:1.76-bookworm
ARG base_image=debian:bookworm
FROM ${build_image} AS build

WORKDIR /build
COPY ./kpi-tracker ./src
COPY ./deps/concordium-rust-sdk ./deps/concordium-rust-sdk

WORKDIR /build/src
RUN cargo build --release

FROM ${base_image}
COPY --from=build /build/src/target/release/concordium-kpi-tracker /usr/local/bin/
ENTRYPOINT [ "concordium-kpi-tracker" ]
