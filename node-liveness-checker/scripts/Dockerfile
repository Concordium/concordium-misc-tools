ARG base_image
FROM ${base_image}
# Needed by run custom build command for 'concordium-rust-sdk'.
RUN rustup component add rustfmt

# Install system dependencies ('cmake' and 'g++' are dependencies of Rust crate 'prost-build').
RUN apt-get update && apt-get install -y libssl-dev pkg-config cmake g++ && rm -rf /var/lib/apt/lists/*

# Install protobuf
RUN wget https://github.com/protocolbuffers/protobuf/releases/download/v3.15.3/protoc-3.15.3-linux-x86_64.zip \
    && unzip protoc-3.15.3-linux-x86_64.zip \
    && mv ./bin/protoc \
    && chmod +x /usr/bin/protoc
