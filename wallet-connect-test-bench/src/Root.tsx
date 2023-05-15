import React from 'react';

import { WithWalletConnector } from '@concordium/react-components';
import Transactions from './Transactions';
import { TESTNET } from './constants';

/**
 * Connect to wallet, setup application state context, and render children when the wallet API is ready for use.
 */
export default function Root() {
    return (
        <div>
            <main className="Transactions">
                <WithWalletConnector network={TESTNET}>{(props) => <Transactions {...props} />}</WithWalletConnector>
            </main>
        </div>
    );
}
