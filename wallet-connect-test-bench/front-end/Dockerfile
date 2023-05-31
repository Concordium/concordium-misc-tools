# This dockerfile is meant to be run from the **root of the repository**.

ARG base_image=node:16-slim

FROM ${base_image} AS build

# Copy front end files
WORKDIR /app
COPY ./package.json ./package.json
COPY ./.yarnrc.yml ./.yarnrc.yml
COPY ./.yarn/releases ./.yarn/releases
COPY ./wallet-connect-test-bench/front-end ./wallet-connect-test-bench/front-end/
COPY ./deps/concordium-dapp-libraries ./deps/concordium-dapp-libraries

# Install front end dependencies and build
RUN yarn
RUN cd ./wallet-connect-test-bench/front-end/ && yarn build

# Serve front end
FROM nginx
COPY --from=build ./app/wallet-connect-test-bench/front-end/dist ./usr/share/nginx/html
