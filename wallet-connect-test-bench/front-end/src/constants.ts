import {
    BrowserWalletConnector,
    ephemeralConnectorType,
    Network,
    WalletConnectConnector,
    CONCORDIUM_WALLET_CONNECT_PROJECT_ID,
} from '@concordium/react-components';
import { SignClientTypes } from '@walletconnect/types';
import moment from 'moment';

export const REFRESH_INTERVAL = moment.duration(10, 'seconds');

// The TESTNET_GENESIS_BLOCK_HASH is used to check that the user has its browser wallet connected to testnet and not to mainnet.
export const TESTNET_GENESIS_BLOCK_HASH = '4221332d34e1694168c2a0c0b3fd0f273809612cb13d000d5c2e00e85f50f796';

// The 'PARAMETER'/'RETURN_VALUE' schemas are created by running the command `cargo concordium --schema-json-out ./` in the `smart-contract` folder.
// This produces an output file in the same folder which those schemas.
export const SET_U8_PARAMETER_SCHEMA = 'Ag==';

export const SET_U16_PARAMETER_SCHEMA = 'Aw==';

export const SET_CONTRACT_ADDRESS_PARAMETER_SCHEMA = 'DA==';

export const SET_ADDRESS_PARAMETER_SCHEMA = 'FQIAAAAHAAAAQWNjb3VudAEBAAAACwgAAABDb250cmFjdAEBAAAADA';

export const SET_ACCOUNT_ADDRESS_PARAMETER_SCHEMA = 'Cw==';

export const SET_HASH_PARAMETER_SCHEMA = 'HiAAAAA=';

export const SET_PUBLIC_KEY_PARAMETER_SCHEMA = 'HiAAAAA=';

export const SET_SIGNATURE_PARAMETER_SCHEMA = 'HkAAAAA=';

export const SET_TIMESTAMP_PARAMETER_SCHEMA = 'DQ==';

export const SET_STRING_PARAMETER_SCHEMA = 'FgI=';

export const SET_OPTION_PARAMETER_SCHEMA = 'FQIAAAAEAAAATm9uZQIEAAAAU29tZQEBAAAAAg==';

export const SET_OBJECT_PARAMETER_SCHEMA =
    'FAAMAAAACAAAAHU4X3ZhbHVlAgkAAAB1MTZfdmFsdWUDDQAAAGFkZHJlc3NfYXJyYXkQAhUCAAAABwAAAEFjY291bnQBAQAAAAsIAAAAQ29udHJhY3QBAQAAAAwNAAAAYWRkcmVzc192YWx1ZRUCAAAABwAAAEFjY291bnQBAQAAAAsIAAAAQ29udHJhY3QBAQAAAAwVAAAAYWNjb3VudF9hZGRyZXNzX3ZhbHVlCxYAAABjb250cmFjdF9hZGRyZXNzX3ZhbHVlDAoAAABoYXNoX3ZhbHVlHiAAAAAPAAAAc2lnbmF0dXJlX3ZhbHVlHkAAAAAQAAAAcHVibGljX2tleV92YWx1ZR4gAAAADwAAAHRpbWVzdGFtcF92YWx1ZQ0MAAAAb3B0aW9uX3ZhbHVlFQIAAAAEAAAATm9uZQIEAAAAU29tZQEBAAAAAgwAAABzdHJpbmdfdmFsdWUWAQ==';

export const SET_ADDRESS_ARRAY_PARAMETER_SCHEMA = 'EAIVAgAAAAcAAABBY2NvdW50AQEAAAALCAAAAENvbnRyYWN0AQEAAAAM';

export const GET_U8_RETURN_VALUE_SCHEMA = 'Ag==';

export const GET_U16_RETURN_VALUE_SCHEMA = 'Aw==';

export const GET_CONTRACT_ADDRESS_RETURN_VALUE_SCHEMA = 'DA==';

export const GET_ADDRESS_RETURN_VALUE_SCHEMA = 'FQIAAAAHAAAAQWNjb3VudAEBAAAACwgAAABDb250cmFjdAEBAAAADA==';

export const GET_ACCOUNT_ADDRESS_RETURN_VALUE_SCHEMA = 'Cw==';

export const GET_HASH_RETURN_VALUE_SCHEMA = 'HiAAAAA=';

export const GET_PUBLIC_KEY_RETURN_VALUE_SCHEMA = 'HiAAAAA=';

export const GET_SIGNATURE_RETURN_VALUE_SCHEMA = 'HkAAAAA=';

export const GET_TIMESTAMP_RETURN_VALUE_SCHEMA = 'DQ==';

export const GET_STRING_RETURN_VALUE_SCHEMA = 'FgI=';

export const GET_OPTION_RETURN_VALUE_SCHEMA = 'FQIAAAAEAAAATm9uZQIEAAAAU29tZQEBAAAAAg==';

export const GET_OBJECT_RETURN_VALUE_SCHEMA =
    'FAAMAAAACAAAAHU4X3ZhbHVlAgkAAAB1MTZfdmFsdWUDDQAAAGFkZHJlc3NfYXJyYXkQAhUCAAAABwAAAEFjY291bnQBAQAAAAsIAAAAQ29udHJhY3QBAQAAAAwNAAAAYWRkcmVzc192YWx1ZRUCAAAABwAAAEFjY291bnQBAQAAAAsIAAAAQ29udHJhY3QBAQAAAAwVAAAAYWNjb3VudF9hZGRyZXNzX3ZhbHVlCxYAAABjb250cmFjdF9hZGRyZXNzX3ZhbHVlDAoAAABoYXNoX3ZhbHVlHiAAAAAPAAAAc2lnbmF0dXJlX3ZhbHVlHkAAAAAQAAAAcHVibGljX2tleV92YWx1ZR4gAAAADwAAAHRpbWVzdGFtcF92YWx1ZQ0MAAAAb3B0aW9uX3ZhbHVlFQIAAAAEAAAATm9uZQIEAAAAU29tZQEBAAAAAgwAAABzdHJpbmdfdmFsdWUWAQ==';

export const GET_ADDRESS_ARRAY_RETURN_VALUE_SCHEMA = 'EAIVAgAAAAcAAABBY2NvdW50AQEAAAALCAAAAENvbnRyYWN0AQEAAAAM';

export const VIEW_RETURN_VALUE_SCHEMA =
    'FAAMAAAACAAAAHU4X3ZhbHVlAgkAAAB1MTZfdmFsdWUDDQAAAGFkZHJlc3NfYXJyYXkQAhUCAAAABwAAAEFjY291bnQBAQAAAAsIAAAAQ29udHJhY3QBAQAAAAwNAAAAYWRkcmVzc192YWx1ZRUCAAAABwAAAEFjY291bnQBAQAAAAsIAAAAQ29udHJhY3QBAQAAAAwVAAAAYWNjb3VudF9hZGRyZXNzX3ZhbHVlCxYAAABjb250cmFjdF9hZGRyZXNzX3ZhbHVlDAoAAABoYXNoX3ZhbHVlHiAAAAAPAAAAc2lnbmF0dXJlX3ZhbHVlHkAAAAAQAAAAcHVibGljX2tleV92YWx1ZR4gAAAADwAAAHRpbWVzdGFtcF92YWx1ZQ0MAAAAb3B0aW9uX3ZhbHVlFQIAAAAEAAAATm9uZQIEAAAAU29tZQEBAAAAAgwAAABzdHJpbmdfdmFsdWUWAQ==';

// The 'BASE_64_SCHEMA' is created by running the command `cargo concordium --schema-base64-out -` in the `smart-contract` folder.
// This command prints the below schema to the console.
export const BASE_64_SCHEMA =
    '//8DAQAAABkAAABzbWFydF9jb250cmFjdF90ZXN0X2JlbmNoACwAAAATAAAAZ2V0X2FjY291bnRfYWRkcmVzcwULFQMAAAALAAAAUGFyc2VQYXJhbXMCFAAAAFNtYXJ0Q29udHJhY3RSZXZlcnRzAgsAAABJbnZva2VFcnJvcgILAAAAZ2V0X2FkZHJlc3MFFQIAAAAHAAAAQWNjb3VudAEBAAAACwgAAABDb250cmFjdAEBAAAADBUDAAAACwAAAFBhcnNlUGFyYW1zAhQAAABTbWFydENvbnRyYWN0UmV2ZXJ0cwILAAAASW52b2tlRXJyb3ICEQAAAGdldF9hZGRyZXNzX2FycmF5BRACFQIAAAAHAAAAQWNjb3VudAEBAAAACwgAAABDb250cmFjdAEBAAAADBUDAAAACwAAAFBhcnNlUGFyYW1zAhQAAABTbWFydENvbnRyYWN0UmV2ZXJ0cwILAAAASW52b2tlRXJyb3ICFAAAAGdldF9jb250cmFjdF9hZGRyZXNzBQwVAwAAAAsAAABQYXJzZVBhcmFtcwIUAAAAU21hcnRDb250cmFjdFJldmVydHMCCwAAAEludm9rZUVycm9yAggAAABnZXRfaGFzaAUeIAAAABUDAAAACwAAAFBhcnNlUGFyYW1zAhQAAABTbWFydENvbnRyYWN0UmV2ZXJ0cwILAAAASW52b2tlRXJyb3ICCgAAAGdldF9vYmplY3QFFAAMAAAACAAAAHU4X3ZhbHVlAgkAAAB1MTZfdmFsdWUDDQAAAGFkZHJlc3NfYXJyYXkQAhUCAAAABwAAAEFjY291bnQBAQAAAAsIAAAAQ29udHJhY3QBAQAAAAwNAAAAYWRkcmVzc192YWx1ZRUCAAAABwAAAEFjY291bnQBAQAAAAsIAAAAQ29udHJhY3QBAQAAAAwVAAAAYWNjb3VudF9hZGRyZXNzX3ZhbHVlCxYAAABjb250cmFjdF9hZGRyZXNzX3ZhbHVlDAoAAABoYXNoX3ZhbHVlHiAAAAAPAAAAc2lnbmF0dXJlX3ZhbHVlHkAAAAAQAAAAcHVibGljX2tleV92YWx1ZR4gAAAADwAAAHRpbWVzdGFtcF92YWx1ZQ0MAAAAb3B0aW9uX3ZhbHVlFQIAAAAEAAAATm9uZQIEAAAAU29tZQEBAAAAAgwAAABzdHJpbmdfdmFsdWUWARUDAAAACwAAAFBhcnNlUGFyYW1zAhQAAABTbWFydENvbnRyYWN0UmV2ZXJ0cwILAAAASW52b2tlRXJyb3ICDQAAAGdldF9vcHRpb25fdTgFFQIAAAAEAAAATm9uZQIEAAAAU29tZQEBAAAAAhUDAAAACwAAAFBhcnNlUGFyYW1zAhQAAABTbWFydENvbnRyYWN0UmV2ZXJ0cwILAAAASW52b2tlRXJyb3ICDgAAAGdldF9wdWJsaWNfa2V5BR4gAAAAFQMAAAALAAAAUGFyc2VQYXJhbXMCFAAAAFNtYXJ0Q29udHJhY3RSZXZlcnRzAgsAAABJbnZva2VFcnJvcgINAAAAZ2V0X3NpZ25hdHVyZQUeQAAAABUDAAAACwAAAFBhcnNlUGFyYW1zAhQAAABTbWFydENvbnRyYWN0UmV2ZXJ0cwILAAAASW52b2tlRXJyb3ICCgAAAGdldF9zdHJpbmcFFgIVAwAAAAsAAABQYXJzZVBhcmFtcwIUAAAAU21hcnRDb250cmFjdFJldmVydHMCCwAAAEludm9rZUVycm9yAg0AAABnZXRfdGltZXN0YW1wBQ0VAwAAAAsAAABQYXJzZVBhcmFtcwIUAAAAU21hcnRDb250cmFjdFJldmVydHMCCwAAAEludm9rZUVycm9yAgcAAABnZXRfdTE2BQMVAwAAAAsAAABQYXJzZVBhcmFtcwIUAAAAU21hcnRDb250cmFjdFJldmVydHMCCwAAAEludm9rZUVycm9yAgYAAABnZXRfdTgFAhUDAAAACwAAAFBhcnNlUGFyYW1zAhQAAABTbWFydENvbnRyYWN0UmV2ZXJ0cwILAAAASW52b2tlRXJyb3ICFQAAAGludGVybmFsX2NhbGxfcmV2ZXJ0cwMVAwAAAAsAAABQYXJzZVBhcmFtcwIUAAAAU21hcnRDb250cmFjdFJldmVydHMCCwAAAEludm9rZUVycm9yAhUAAABpbnRlcm5hbF9jYWxsX3N1Y2Nlc3MDFQMAAAALAAAAUGFyc2VQYXJhbXMCFAAAAFNtYXJ0Q29udHJhY3RSZXZlcnRzAgsAAABJbnZva2VFcnJvcgIHAAAAcmV2ZXJ0cwMVAwAAAAsAAABQYXJzZVBhcmFtcwIUAAAAU21hcnRDb250cmFjdFJldmVydHMCCwAAAEludm9rZUVycm9yAhMAAABzZXRfYWNjb3VudF9hZGRyZXNzBAsVAwAAAAsAAABQYXJzZVBhcmFtcwIUAAAAU21hcnRDb250cmFjdFJldmVydHMCCwAAAEludm9rZUVycm9yAhsAAABzZXRfYWNjb3VudF9hZGRyZXNzX3BheWFibGUECxUDAAAACwAAAFBhcnNlUGFyYW1zAhQAAABTbWFydENvbnRyYWN0UmV2ZXJ0cwILAAAASW52b2tlRXJyb3ICCwAAAHNldF9hZGRyZXNzBBUCAAAABwAAAEFjY291bnQBAQAAAAsIAAAAQ29udHJhY3QBAQAAAAwVAwAAAAsAAABQYXJzZVBhcmFtcwIUAAAAU21hcnRDb250cmFjdFJldmVydHMCCwAAAEludm9rZUVycm9yAhEAAABzZXRfYWRkcmVzc19hcnJheQQQAhUCAAAABwAAAEFjY291bnQBAQAAAAsIAAAAQ29udHJhY3QBAQAAAAwVAwAAAAsAAABQYXJzZVBhcmFtcwIUAAAAU21hcnRDb250cmFjdFJldmVydHMCCwAAAEludm9rZUVycm9yAhkAAABzZXRfYWRkcmVzc19hcnJheV9wYXlhYmxlBBACFQIAAAAHAAAAQWNjb3VudAEBAAAACwgAAABDb250cmFjdAEBAAAADBUDAAAACwAAAFBhcnNlUGFyYW1zAhQAAABTbWFydENvbnRyYWN0UmV2ZXJ0cwILAAAASW52b2tlRXJyb3ICEwAAAHNldF9hZGRyZXNzX3BheWFibGUEFQIAAAAHAAAAQWNjb3VudAEBAAAACwgAAABDb250cmFjdAEBAAAADBUDAAAACwAAAFBhcnNlUGFyYW1zAhQAAABTbWFydENvbnRyYWN0UmV2ZXJ0cwILAAAASW52b2tlRXJyb3ICFAAAAHNldF9jb250cmFjdF9hZGRyZXNzBAwVAwAAAAsAAABQYXJzZVBhcmFtcwIUAAAAU21hcnRDb250cmFjdFJldmVydHMCCwAAAEludm9rZUVycm9yAhwAAABzZXRfY29udHJhY3RfYWRkcmVzc19wYXlhYmxlBAwVAwAAAAsAAABQYXJzZVBhcmFtcwIUAAAAU21hcnRDb250cmFjdFJldmVydHMCCwAAAEludm9rZUVycm9yAggAAABzZXRfaGFzaAQeIAAAABUDAAAACwAAAFBhcnNlUGFyYW1zAhQAAABTbWFydENvbnRyYWN0UmV2ZXJ0cwILAAAASW52b2tlRXJyb3ICEAAAAHNldF9oYXNoX3BheWFibGUEHiAAAAAVAwAAAAsAAABQYXJzZVBhcmFtcwIUAAAAU21hcnRDb250cmFjdFJldmVydHMCCwAAAEludm9rZUVycm9yAgoAAABzZXRfb2JqZWN0BBQADAAAAAgAAAB1OF92YWx1ZQIJAAAAdTE2X3ZhbHVlAw0AAABhZGRyZXNzX2FycmF5EAIVAgAAAAcAAABBY2NvdW50AQEAAAALCAAAAENvbnRyYWN0AQEAAAAMDQAAAGFkZHJlc3NfdmFsdWUVAgAAAAcAAABBY2NvdW50AQEAAAALCAAAAENvbnRyYWN0AQEAAAAMFQAAAGFjY291bnRfYWRkcmVzc192YWx1ZQsWAAAAY29udHJhY3RfYWRkcmVzc192YWx1ZQwKAAAAaGFzaF92YWx1ZR4gAAAADwAAAHNpZ25hdHVyZV92YWx1ZR5AAAAAEAAAAHB1YmxpY19rZXlfdmFsdWUeIAAAAA8AAAB0aW1lc3RhbXBfdmFsdWUNDAAAAG9wdGlvbl92YWx1ZRUCAAAABAAAAE5vbmUCBAAAAFNvbWUBAQAAAAIMAAAAc3RyaW5nX3ZhbHVlFgEVAwAAAAsAAABQYXJzZVBhcmFtcwIUAAAAU21hcnRDb250cmFjdFJldmVydHMCCwAAAEludm9rZUVycm9yAhIAAABzZXRfb2JqZWN0X3BheWFibGUEFAAMAAAACAAAAHU4X3ZhbHVlAgkAAAB1MTZfdmFsdWUDDQAAAGFkZHJlc3NfYXJyYXkQAhUCAAAABwAAAEFjY291bnQBAQAAAAsIAAAAQ29udHJhY3QBAQAAAAwNAAAAYWRkcmVzc192YWx1ZRUCAAAABwAAAEFjY291bnQBAQAAAAsIAAAAQ29udHJhY3QBAQAAAAwVAAAAYWNjb3VudF9hZGRyZXNzX3ZhbHVlCxYAAABjb250cmFjdF9hZGRyZXNzX3ZhbHVlDAoAAABoYXNoX3ZhbHVlHiAAAAAPAAAAc2lnbmF0dXJlX3ZhbHVlHkAAAAAQAAAAcHVibGljX2tleV92YWx1ZR4gAAAADwAAAHRpbWVzdGFtcF92YWx1ZQ0MAAAAb3B0aW9uX3ZhbHVlFQIAAAAEAAAATm9uZQIEAAAAU29tZQEBAAAAAgwAAABzdHJpbmdfdmFsdWUWARUDAAAACwAAAFBhcnNlUGFyYW1zAhQAAABTbWFydENvbnRyYWN0UmV2ZXJ0cwILAAAASW52b2tlRXJyb3ICDQAAAHNldF9vcHRpb25fdTgEFQIAAAAEAAAATm9uZQIEAAAAU29tZQEBAAAAAhUDAAAACwAAAFBhcnNlUGFyYW1zAhQAAABTbWFydENvbnRyYWN0UmV2ZXJ0cwILAAAASW52b2tlRXJyb3ICFQAAAHNldF9vcHRpb25fdThfcGF5YWJsZQQVAgAAAAQAAABOb25lAgQAAABTb21lAQEAAAACFQMAAAALAAAAUGFyc2VQYXJhbXMCFAAAAFNtYXJ0Q29udHJhY3RSZXZlcnRzAgsAAABJbnZva2VFcnJvcgIOAAAAc2V0X3B1YmxpY19rZXkEHiAAAAAVAwAAAAsAAABQYXJzZVBhcmFtcwIUAAAAU21hcnRDb250cmFjdFJldmVydHMCCwAAAEludm9rZUVycm9yAhYAAABzZXRfcHVibGljX2tleV9wYXlhYmxlBB4gAAAAFQMAAAALAAAAUGFyc2VQYXJhbXMCFAAAAFNtYXJ0Q29udHJhY3RSZXZlcnRzAgsAAABJbnZva2VFcnJvcgINAAAAc2V0X3NpZ25hdHVyZQQeQAAAABUDAAAACwAAAFBhcnNlUGFyYW1zAhQAAABTbWFydENvbnRyYWN0UmV2ZXJ0cwILAAAASW52b2tlRXJyb3ICFQAAAHNldF9zaWduYXR1cmVfcGF5YWJsZQQeQAAAABUDAAAACwAAAFBhcnNlUGFyYW1zAhQAAABTbWFydENvbnRyYWN0UmV2ZXJ0cwILAAAASW52b2tlRXJyb3ICCgAAAHNldF9zdHJpbmcEFgIVAwAAAAsAAABQYXJzZVBhcmFtcwIUAAAAU21hcnRDb250cmFjdFJldmVydHMCCwAAAEludm9rZUVycm9yAhIAAABzZXRfc3RyaW5nX3BheWFibGUEFgIVAwAAAAsAAABQYXJzZVBhcmFtcwIUAAAAU21hcnRDb250cmFjdFJldmVydHMCCwAAAEludm9rZUVycm9yAg0AAABzZXRfdGltZXN0YW1wBA0VAwAAAAsAAABQYXJzZVBhcmFtcwIUAAAAU21hcnRDb250cmFjdFJldmVydHMCCwAAAEludm9rZUVycm9yAhUAAABzZXRfdGltZXN0YW1wX3BheWFibGUEDRUDAAAACwAAAFBhcnNlUGFyYW1zAhQAAABTbWFydENvbnRyYWN0UmV2ZXJ0cwILAAAASW52b2tlRXJyb3ICBwAAAHNldF91MTYEAxUDAAAACwAAAFBhcnNlUGFyYW1zAhQAAABTbWFydENvbnRyYWN0UmV2ZXJ0cwILAAAASW52b2tlRXJyb3ICDwAAAHNldF91MTZfcGF5YWJsZQQDFQMAAAALAAAAUGFyc2VQYXJhbXMCFAAAAFNtYXJ0Q29udHJhY3RSZXZlcnRzAgsAAABJbnZva2VFcnJvcgIGAAAAc2V0X3U4BAIVAwAAAAsAAABQYXJzZVBhcmFtcwIUAAAAU21hcnRDb250cmFjdFJldmVydHMCCwAAAEludm9rZUVycm9yAg4AAABzZXRfdThfcGF5YWJsZQQCFQMAAAALAAAAUGFyc2VQYXJhbXMCFAAAAFNtYXJ0Q29udHJhY3RSZXZlcnRzAgsAAABJbnZva2VFcnJvcgIHAAAAc3VjY2VzcwMVAwAAAAsAAABQYXJzZVBhcmFtcwIUAAAAU21hcnRDb250cmFjdFJldmVydHMCCwAAAEludm9rZUVycm9yAgQAAAB2aWV3Bh4gAAAAFAAMAAAACAAAAHU4X3ZhbHVlAgkAAAB1MTZfdmFsdWUDDQAAAGFkZHJlc3NfYXJyYXkQAhUCAAAABwAAAEFjY291bnQBAQAAAAsIAAAAQ29udHJhY3QBAQAAAAwNAAAAYWRkcmVzc192YWx1ZRUCAAAABwAAAEFjY291bnQBAQAAAAsIAAAAQ29udHJhY3QBAQAAAAwVAAAAYWNjb3VudF9hZGRyZXNzX3ZhbHVlCxYAAABjb250cmFjdF9hZGRyZXNzX3ZhbHVlDAoAAABoYXNoX3ZhbHVlHiAAAAAPAAAAc2lnbmF0dXJlX3ZhbHVlHkAAAAAQAAAAcHVibGljX2tleV92YWx1ZR4gAAAADwAAAHRpbWVzdGFtcF92YWx1ZQ0MAAAAb3B0aW9uX3ZhbHVlFQIAAAAEAAAATm9uZQIEAAAAU29tZQEBAAAAAgwAAABzdHJpbmdfdmFsdWUWARUDAAAACwAAAFBhcnNlUGFyYW1zAhQAAABTbWFydENvbnRyYWN0UmV2ZXJ0cwILAAAASW52b2tlRXJyb3ICAA==';

export const CONTRACT_NAME = 'smart_contract_test_bench';

export const CONTRACT_INDEX = 4568n;

export const CONTRACT_SUB_INDEX = 0n;

const WALLET_CONNECT_OPTS: SignClientTypes.Options = {
    projectId: CONCORDIUM_WALLET_CONNECT_PROJECT_ID,
    metadata: {
        name: 'Test_Bench',
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
