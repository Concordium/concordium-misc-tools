# This dockerfile is meant to be run from the **root of the repository**.

ARG build_image=node:16-slim

FROM ${build_image} AS build

RUN yarn set version 3.2.0

# Copy front end files
WORKDIR /app
COPY ./wallet-connect-test-bench/front-end-with-dapp-library ./wallet-connect-test-bench/front-end-with-dapp-library/
COPY ./deps/concordium-dapp-libraries ./deps/concordium-dapp-libraries

# Install front end dependencies
WORKDIR /app/deps/concordium-dapp-libraries
RUN yarn install && yarn build

WORKDIR /app/wallet-connect-test-bench/front-end-with-dapp-library
RUN yarn install && yarn build

# Serve front end
FROM nginx
COPY --from=build ./app/wallet-connect-test-bench/front-end-with-dapp-library/dist ./usr/share/nginx/html
