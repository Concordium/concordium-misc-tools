ARG build_image=rust:1.70-buster
ARG base_image=debian:buster-slim
FROM ${build_image} AS build

WORKDIR /usr/app/recover-id-object

COPY recover-id-object .
COPY deps /usr/app/deps
RUN cargo build --release --locked

FROM ${base_image}

WORKDIR /usr/app

COPY --from=build /usr/app/recover-id-object/target/release/recover-id-object ./recover-id-object

RUN apt-get update && \
    apt-get -y install \
    # curl is used to report errors to slack with
      ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

RUN chmod +x recover-id-object

ENTRYPOINT ["./recover-id-object"]
