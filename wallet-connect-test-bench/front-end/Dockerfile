ARG base_image=node:16-slim

FROM ${base_image} AS build

# Copy front end files
WORKDIR /app
COPY ./package.json ./front-end/
COPY ./tsconfig.json ./front-end/
COPY ./esbuild.config.ts ./front-end/
COPY ./src ./front-end/src
COPY ./yarn.lock ./front-end/

# Remove local preinstall command in `package.json` file which causes errors since the patch does not exist in the container
RUN cd ./front-end/ && sed -i '/preinstall/d' package.json

# Install git
RUN apt-get update && \
    apt-get upgrade -y && \
    apt-get install -y git

# Build dapp libraries
RUN git clone https://github.com/Concordium/concordium-dapp-libraries.git
RUN cd ./concordium-dapp-libraries/ && yarn && yarn build

# Install front end dependencies
RUN cd ./front-end/ && yarn && yarn cache clean

# Copy dapp libaries into front end `node_modules` folder
RUN cp -r ./concordium-dapp-libraries/packages/react-components ./front-end/node_modules/@concordium/react-components; cp -r ./concordium-dapp-libraries/packages/wallet-connectors ./front-end/node_modules/@concordium/wallet-connectors

# Build front end
RUN cd ./front-end/ && yarn build

# Serve front end
FROM nginx
COPY --from=build ./app/front-end/dist ./usr/share/nginx/html
