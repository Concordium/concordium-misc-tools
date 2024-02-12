FROM rust:1.68.2 AS build

WORKDIR /usr/app/recover-id-object

COPY recover-id-object/Cargo.toml recover-id-object/Cargo.lock .
COPY deps /usr/app/deps
RUN mkdir src && echo 'fn main() { println!("Dummy!"); }' > ./src/main.rs

RUN cargo build --release --locked
RUN rm recover-id-object/src/*.rs
COPY recover-id-object/src ./src

#RUN rm ./target/release/deps/pokemon_api*
#RUN cargo build --release --locked
#
#FROM ${base_image}
#RUN apt-get update && \
#    apt-get -y install \
#      ca-certificates \
#    && rm -rf /var/lib/apt/lists/*
#COPY --from=build /build/chain-prometheus-exporter/target/release/chain-prometheus-exporter /usr/local/bin/

