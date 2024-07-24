ARG build_image=rust:1.74-buster
ARG base_image=debian:buster-slim
FROM ${build_image} AS build

WORKDIR /usr/app/notification-server

COPY notification-server/Cargo.toml notification-server/Cargo.lock ./
COPY deps /usr/app/deps
RUN mkdir -p src/bin
RUN echo 'fn main() { println!("Dummy!"); }' | tee ./src/bin/api.rs ./src/bin/service.rs
RUN cargo build --release --locked
RUN rm src/bin/*.rs

COPY notification-server/src ./src
RUN cargo build --release --locked

FROM ${base_image}

WORKDIR /usr/app

COPY --from=build /usr/app/notification-server/target/release/notification-api /usr/app/notification-server/target/release/notification-service ./

RUN chmod +x notification-api notification-service