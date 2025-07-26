ARG build_image=rust:1.70-bookworm
ARG base_image=debian:bookworm-slim
FROM ${build_image} AS build

WORKDIR /usr/app/recover-id-object
COPY . .
RUN cargo build --release --locked -p recover-id-object

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
