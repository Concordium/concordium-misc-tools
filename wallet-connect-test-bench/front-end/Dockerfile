# This dockerfile is meant to be run from the **root of the repository**.

ARG build_image=node:20-slim

FROM ${build_image} AS build

# Copy front end files
WORKDIR /app
COPY ./wallet-connect-test-bench/front-end ./wallet-connect-test-bench/front-end/
COPY ./deps/concordium-dapp-libraries ./deps/concordium-dapp-libraries

# Install front end dependencies
WORKDIR /app/deps/concordium-dapp-libraries
RUN yarn install && yarn build

WORKDIR /app/wallet-connect-test-bench/front-end
RUN yarn install && yarn build

# Serve front end
FROM nginx
COPY --from=build ./app/wallet-connect-test-bench/front-end/dist ./usr/share/nginx/html
