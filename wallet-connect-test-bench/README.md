# Test Bench

A test bench front end for testing various positive/negative scenarios of how the wallets could interact with a smart contract using either a mobile wallet (using wallet connect) or the browser wallet. The front end displays all responses (e.g. error messages returned as well as transaction hashes returned) for further investigation.

The project has three folders (the `smart-contract` folder, the `front-end` folder, and the `front-end-with-dapp-library` folder). 

The `front-end-with-dapp-library` folder uses the dApp libraries to connect to the walltes, while the `front-end` folder calls the wallet API without any intermediate libraries.
The `front-end-with-dapp-library` uses the [react-components/wallet-connectors libraries](https://github.com/Concordium/concordium-dapp-libraries/tree/main/packages) to create the connection between the wallets and the front end. 

