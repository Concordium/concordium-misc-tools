import React from 'react';

import Main from './Main';

/**
 * Connect to wallet, setup application state context, and render children when the wallet API is ready for use.
 */
export default function Root() {
    return (
        <div>
            <main>
                <Main />
            </main>
        </div>
    );
}
