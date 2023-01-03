import { detectConcordiumProvider } from '@concordium/browser-wallet-api-helpers';
import { IdProofOutput, IdStatement } from '@concordium/web-sdk';
import { SessionTypes, SignClientTypes } from '@walletconnect/types';
import SignClient from '@walletconnect/sign-client';
import QRCodeModal from '@walletconnect/qrcode-modal';
import EventEmitter from 'events';

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

export abstract class WalletProvider extends EventEmitter {
    abstract connect(): Promise<string | undefined>;
    abstract requestIdProof(statement: IdStatement, challenge: string): Promise<IdProofOutput>;
    disconnect?(): Promise<void>;

    /**
     * @param account string when account is changed, undefined when disconnected
     */
    protected onAccountChanged(account: string | undefined) {
        this.emit('accountChanged', account);
    }
}

export class BrowserWalletProvider extends WalletProvider {
    async connect(): Promise<string | undefined> {
        const provider = await detectConcordiumProvider();

        provider.on('accountChanged', (account) => super.onAccountChanged(account));
        provider.on('accountDisconnected', async () =>
            super.onAccountChanged((await provider.getMostRecentlySelectedAccount()) ?? undefined)
        );

        // Check if you are already connected
        const selectedAccount = await provider.getMostRecentlySelectedAccount();
        return selectedAccount ?? provider.connect();
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

const ID_METHOD = 'proof_of_identity';

export class WalletConnectProvider extends WalletProvider {
    private account: string | undefined;
    private topic: string | undefined;

    constructor(private client: SignClient) {
        super();

        this.client.on('session_update', ({ params }) => {
            this.account = this.getAccount(params.namespaces);
            super.onAccountChanged(this.account);
        });

        this.client.on('session_delete', () => {
            this.account = undefined;
            this.topic = undefined;

            super.onAccountChanged(this.account);
        });
    }

    static async create() {
        const client = await SignClient.init(walletConnectOpts);
        return new WalletConnectProvider(client);
    }

    async connect(): Promise<string | undefined> {
        try {
            const { uri, approval } = await this.client.connect({
                requiredNamespaces: {
                    [WALLET_CONNECT_SESSION_NAMESPACE]: {
                        methods: [ID_METHOD],
                        chains: [CHAIN_ID],
                        events: ['accounts_changed'],
                    },
                },
            });
            // Open QRCode modal if a URI was returned (i.e. we're not connecting an existing pairing).
            if (uri) {
                QRCodeModal.open(uri, undefined);
            }
            // Await session en approval from the wallet.
            const session = await approval();

            this.account = this.getAccount(session.namespaces);
            this.topic = session.topic;

            return this.account;
        } finally {
            // Close the QRCode modal in case it was open.
            QRCodeModal.close();
        }
    }

    async requestIdProof(statement: IdStatement, challenge: string): Promise<IdProofOutput> {
        if (!this.topic) {
            throw new Error('No connection');
        }

        const params = {
            accountAddress: this.account,
            statement,
            challenge,
        };

        const { idProof } = (await this.client.request({
            topic: this.topic,
            request: {
                method: ID_METHOD,
                params,
            },
            chainId: CHAIN_ID,
        })) as { idProof: IdProofOutput };

        return idProof;
    }

    async disconnect(): Promise<void> {
        if (this.topic === undefined) {
            return;
        }

        await this.client.disconnect({
            topic: this.topic,
            reason: {
                code: 1,
                message: 'user disconnecting',
            },
        });

        this.account = undefined;
        this.topic = undefined;

        super.onAccountChanged(this.account);
    }

    private getAccount(ns: SessionTypes.Namespaces): string | undefined {
        const [, , account] = ns[WALLET_CONNECT_SESSION_NAMESPACE].accounts[0].split(':');
        return account;
    }
}
