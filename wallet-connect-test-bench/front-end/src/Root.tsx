import React from 'react';

import { WithWalletConnector, TESTNET } from '@concordium/react-components';
import Main from './Main';

/**
 * Connect to wallet, setup application state context, and render children when the wallet API is ready for use.
 */
export default function Root() {
    return (
        <div>
            <main>
                <WithWalletConnector network={TESTNET}>{(props) => <Main {...props} />}</WithWalletConnector>
            </main>
        </div>
    );
}
