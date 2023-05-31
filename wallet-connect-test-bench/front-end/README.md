# Test Bench

A test bench for testing mobile wallets (via walletConnect) or the browser wallet.

## Prerequisites

-   Browser wallet extension must be installed in Chrome browser and the Concordium testnet needs to be selected or a mobile wallet needs to be set up that supports walletConnect in order to view smart contract details or submit transactions.

## Running the test bench front-end

Clone the repo:

```shell
git clone --recursive-submodules git@github.com:Concordium/concordium-misc-tools
```

Navigate into ./deps/concordium-dapplibraries and build the dApp libraries packages:

```shell
cd ./deps/concordium-dapp-libraries/
yarn
yarn build
```

Navigate into this folder:
```shell
cd ../wallet-connect-test-bench/front-end
```

-   Run `yarn install` in this folder.
-   Run `yarn build` in a terminal in this folder.
-   Run `yarn start`.
-   Open URL logged in console (typically http://127.0.0.1:8080).

To have hot-reload (useful for development), do the following instead:

-   Run `yarn watch` in a terminal.
-   Run `yarn start` in another terminal.
-   Open URL logged in console (typically http://127.0.0.1:8080).

## Using yarn (on unix/macOS systems)
Some of the node modules have Windows-type line endings (\r\n), instead of unix line endings (\n), which causes problems when using an old yarn package manager.

If you see an error message similar to this when executing `yarn start`, then you've run into the problem:
```shell
env: node\r: No such file or directory
```

Use `npm install` instead of `yarn install` in the above command or use an up-to-date `yarn` version. `npm` (newer `yarn` version) will correct the line ending.

Additional information can be found [here](https://techtalkbook.com/env-noder-no-such-file-or-directory/).

## Build and run the Docker image

To build the docker image run the following command **from the root of the repository**:

```
docker build -f wallet-connect-test-bench/front-end/Dockerfile -t test_bench:$PROJECT_VERSION .
```

e.g.

```
docker build -f wallet-connect-test-bench/front-end/Dockerfile -t test_bench:3.0.0 .
```

To run the docker image run the following command:

```
docker run -it -d -p 8080:80 --name web test_bench:$PROJECT_VERSION
```

e.g.

```
docker run -it -d -p 8080:80 --name web test_bench:3.0.0
```

Open http://127.0.0.1:8080 in your browser.

