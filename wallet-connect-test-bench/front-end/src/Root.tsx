import { WithWalletConnector, TESTNET } from "@concordium/react-components";
import { STAGENET } from "@concordium/wallet-connectors";
import { Network } from "@concordium/react-components";

/**
 * Connect to wallet, setup application state context, and render children when the wallet API is ready for use.
 */
import React, { useState } from "react";
import ReactDOM from "react-dom/client";

import Main from "./Main";
import "./index.css";

function App() {
  const [network, setNetwork] = useState<Network>(STAGENET);

  return (
    <WithWalletConnector network={network}>
      {(props) => (
        <Main {...props} network={network} setNetwork={setNetwork} />
      )}
    </WithWalletConnector>
  );
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
