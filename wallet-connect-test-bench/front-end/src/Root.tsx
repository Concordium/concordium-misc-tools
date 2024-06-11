import { WithWalletConnector, TESTNET } from "@concordium/react-components";

/**
 * Connect to wallet, setup application state context, and render children when the wallet API is ready for use.
 */
import React from "react";
import ReactDOM from "react-dom/client";

import Main from "./Main";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <WithWalletConnector network={TESTNET}>
      {(props) => <Main {...props} />}
    </WithWalletConnector>
  </React.StrictMode>
);
