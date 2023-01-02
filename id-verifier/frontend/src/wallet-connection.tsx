import { detectConcordiumProvider } from '@concordium/browser-wallet-api-helpers';
import { IdProofOutput, IdStatement } from '@concordium/web-sdk';
import { SignClientTypes } from '@walletconnect/types';
import SignClient from '@walletconnect/sign-client';
import QRCodeModal from '@walletconnect/qrcode-modal';

const WALLET_CONNECT_PROJECT_ID = '76324905a70fe5c388bab46d3e0564dc';
const WALLET_CONNECT_SESSION_NAMESPACE = 'ccd';
const CHAIN_ID = `${WALLET_CONNECT_SESSION_NAMESPACE}:testnet`;

const walletConnectOpts: SignClientTypes.Options = {
    projectId: WALLET_CONNECT_PROJECT_ID,
    metadata: {
        name: 'Proof explorer',
        description: 'Application for testing ID proofs',
        url: '#',
        icons: ['https://walletconnect.com/walletconnect-logo.png'],
    },
};

export interface WalletProvider {
    connect(): Promise<string | undefined>;
    requestIdProof(statement: IdStatement, challenge: string): Promise<IdProofOutput>;
}

export class BrowserWalletProvider implements WalletProvider {
    async connect(): Promise<string | undefined> {
        const provider = await detectConcordiumProvider();
        return provider.connect();
    }

    async requestIdProof(statement: IdStatement, challenge: string): Promise<IdProofOutput> {
        const provider = await detectConcordiumProvider();
        const account = await provider.connect();

        if (!account) {
            throw new Error('No account available');
        }

        return provider.requestIdProof(account, statement, challenge);
    }
}

export class WalletConnectProvider implements WalletProvider {
    private account: string | undefined;

    constructor(private client: SignClient) {}

    static async create() {
        const client = await SignClient.init(walletConnectOpts);
        return new WalletConnectProvider(client);
    }

    async connect(): Promise<string | undefined> {
        try {
            const { uri, approval } = await this.client.connect({
                requiredNamespaces: {
                    ccd: {
                        methods: ['sign_and_send_transaction'], // TODO: find the correct method.
                        chains: [CHAIN_ID],
                        events: ['chain_changed', 'accounts_changed'],
                    },
                },
            });
            // Open QRCode modal if a URI was returned (i.e. we're not connecting an existing pairing).
            if (uri) {
                QRCodeModal.open(uri, undefined);
            }
            // Await session approval from the wallet.
            const session = await approval();
            this.account = session.namespaces[WALLET_CONNECT_SESSION_NAMESPACE].accounts[0];
            return this.account;
        } finally {
            // Close the QRCode modal in case it was open.
            QRCodeModal.close();
        }
    }
    requestIdProof(statement: IdStatement, challenge: string): Promise<IdProofOutput> {
        throw new Error('Method not implemented.');
    }
}
