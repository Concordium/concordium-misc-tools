## Build Phase
FROM rust:1.91-slim as build
WORKDIR /build
COPY . .
RUN ls
RUN cargo build --locked -p credential-verification-service --release

## Run phase
FROM chainguard/glibc-dynamic:latest
COPY --from=build /build/target/release/credential-verification-service /usr/local/bin/
ENTRYPOINT [ "credential-verification-service" ]
