ARG build_image=rust:1.76-buster
ARG base_image=debian:buster-slim
FROM ${build_image} AS build

WORKDIR /usr/app/notification-server

COPY notification-server/Cargo.toml notification-server/Cargo.lock ./
COPY notification-server/src ./src
COPY deps /usr/app/deps
RUN cargo build --release --locked

FROM ${base_image}

WORKDIR /usr/app

RUN apt-get update && \
    apt-get -y install \
      postgresql-client \
      ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY notification-server/resources /usr/app/resources
COPY --from=build /usr/app/notification-server/target/release/notification-api /usr/app/notification-server/target/release/notification-service /usr/bin/

RUN chmod +x /usr/bin/notification-api /usr/bin/notification-service
