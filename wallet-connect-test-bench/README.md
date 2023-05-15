# Test Bench

A test bench for testing wallet Connect or the browser wallet.

## Prerequisites

-   Browser wallet extension must be installed in Google Chrome and the Concordium testnet needs to be selected or a mobile wallet needs to be set up that supports wallet connect in order to view smart contract details or submit transactions.

## Running the sponsored txs example (without backend -> submitting the sponsored transaction to chain will fail)

-   Run `yarn install` in this folder.
-   Run `yarn build` in a terminal in this folder.
-   Run `yarn start`.
-   Open URL logged in console (typically http://127.0.0.1:8080).

To have hot-reload (useful for development), do the following instead:

-   Run `yarn watch` in a terminal.
-   Run `yarn start` in another terminal.
-   Open URL logged in console (typically http://127.0.0.1:8080).
