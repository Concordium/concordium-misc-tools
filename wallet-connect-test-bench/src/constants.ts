// The TESTNET_GENESIS_BLOCK_HASH is used to check that the user has its browser wallet connected to testnet and not to mainnet.
import {
    BrowserWalletConnector,
    ephemeralConnectorType,
    Network,
    WalletConnectConnector,
} from '@concordium/react-components';
import { SignClientTypes } from '@walletconnect/types';
import moment from 'moment';

export const REFRESH_INTERVAL = moment.duration(10, 'seconds');

export const TESTNET_GENESIS_BLOCK_HASH = '4221332d34e1694168c2a0c0b3fd0f273809612cb13d000d5c2e00e85f50f796';

export const TX_CONTRACT_NAME = 'smart_contract_test_bench';

export const SET_U8_PARAMETER_SCHEMA = 'Ag==';

export const TX_CONTRACT_INDEX = 4537n;

export const BASE_64_SCHEMA = '//8DAQAAABkAAABzbWFydF9jb250cmFjdF90ZXN0X2JlbmNoAAMAAAAFAAAAc2V0VTgEAhUBAAAACwAAAFBhcnNlUGFyYW1zAg4AAABzZXRfdThfcGF5YWJsZQQCFQEAAAALAAAAUGFyc2VQYXJhbXMCBAAAAHZpZXcGHiAAAAAUAAEAAAAIAAAAdThfdmFsdWUCFQEAAAALAAAAUGFyc2VQYXJhbXMCAA==';

export const VIEW_RETURN_VALUE_SCHEMA = 'FAABAAAACAAAAHU4X3ZhbHVlAg';

export const CONTRACT_SUB_INDEX = 0n;

const WALLET_CONNECT_PROJECT_ID = '76324905a70fe5c388bab46d3e0564dc';
const WALLET_CONNECT_OPTS: SignClientTypes.Options = {
    projectId: WALLET_CONNECT_PROJECT_ID,
    metadata: {
        name: 'Txs',
        description: 'Example dApp for testing.',
        url: '#',
        icons: ['https://walletconnect.com/walletconnect-logo.png'],
    },
};
export const TESTNET: Network = {
    name: 'testnet',
    genesisHash: TESTNET_GENESIS_BLOCK_HASH,
    jsonRpcUrl: 'https://json-rpc.testnet.concordium.com',
    ccdScanBaseUrl: 'https://testnet.ccdscan.io',
};

export const BROWSER_WALLET = ephemeralConnectorType(BrowserWalletConnector.create);
export const WALLET_CONNECT = ephemeralConnectorType(
    WalletConnectConnector.create.bind(undefined, WALLET_CONNECT_OPTS)
);
