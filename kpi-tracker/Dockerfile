ARG build_image
ARG base_image
FROM ${build_image} AS build
# Install protoc
RUN wget https://github.com/protocolbuffers/protobuf/releases/download/v3.15.3/protoc-3.15.3-linux-x86_64.zip
RUN unzip protoc-3.15.3-linux-x86_64.zip
RUN mv ./bin/protoc /usr/bin/protoc

WORKDIR /build
COPY ./kpi-tracker ./src
COPY ./deps/concordium-rust-sdk ./deps/concordium-rust-sdk

WORKDIR /build/src
RUN cargo build --release

FROM ${base_image}
COPY --from=build /build/src/target/release/concordium-kpi-tracker /usr/local/bin/
ENTRYPOINT [ "concordium-kpi-tracker" ]
