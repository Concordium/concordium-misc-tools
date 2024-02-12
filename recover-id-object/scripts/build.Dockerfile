FROM rust:1.70.0 AS build

WORKDIR /usr/app/recover-id-object

COPY recover-id-object/Cargo.toml recover-id-object/Cargo.lock .
COPY deps /usr/app/deps
RUN mkdir src && echo 'fn main() { println!("Dummy!"); }' > ./src/main.rs

RUN cargo build --release --locked

RUN rm src/*.rs
COPY recover-id-object/src ./src
RUN cargo build --release --locked

FROM debian:buster-slim

WORKDIR /usr/app

COPY --from=build /usr/app/recover-id-object/target/release/recover-id-object ./recover-id-object

RUN groupadd -g 10001 appuser && \
   useradd --system --no-create-home -u 10000 -g appuser appuser \
   && chown -R appuser:appuser /usr/app

USER appuser:appuser

ENTRYPOINT recover-id-object
