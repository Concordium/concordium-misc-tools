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
-   Run `cp -r ../../deps/concordium-dapp-libraries/packages/react-components ./node_modules/@concordium/react-components` in this folder.
-   Run `cp -r ../../deps/concordium-dapp-libraries/packages/wallet-connectors ./node_modules/@concordium/wallet-connectors` in this folder.
-   Run `yarn build` in a terminal in this folder.
-   Run `yarn start`.
-   Open URL logged in console (typically http://127.0.0.1:8080).

To have hot-reload (useful for development), do the following instead:

-   Run `yarn watch` in a terminal.
-   Run `yarn start` in another terminal.
-   Open URL logged in console (typically http://127.0.0.1:8080).
