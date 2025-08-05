ARG build_image=rust:1.76-bookworm
ARG base_image=debian:bookworm
FROM ${build_image} AS build

WORKDIR /build
COPY . .
RUN ls
RUN cargo build --locked -p concordium-kpi-tracker --release

FROM ${base_image}
COPY --from=build /build/target/release/concordium-kpi-tracker /usr/local/bin/
ENTRYPOINT [ "concordium-kpi-tracker" ]
