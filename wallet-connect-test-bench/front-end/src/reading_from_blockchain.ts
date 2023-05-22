import { toBuffer, JsonRpcClient, deserializeTypeValue, deserializeReceiveReturnValue } from '@concordium/web-sdk';

import {
    CONTRACT_NAME,
    CONTRACT_INDEX,
    CONTRACT_SUB_INDEX,
    VIEW_RETURN_VALUE_SCHEMA,
    GET_U8_RETURN_VALUE_SCHEMA,
    GET_U16_RETURN_VALUE_SCHEMA,
    GET_CONTRACT_ADDRESS_RETURN_VALUE_SCHEMA,
    GET_ADDRESS_RETURN_VALUE_SCHEMA,
    GET_ACCOUNT_ADDRESS_RETURN_VALUE_SCHEMA,
    GET_HASH_RETURN_VALUE_SCHEMA,
    GET_PUBLIC_KEY_RETURN_VALUE_SCHEMA,
    GET_SIGNATURE_RETURN_VALUE_SCHEMA,
    GET_TIMESTAMP_RETURN_VALUE_SCHEMA,
    GET_STRING_RETURN_VALUE_SCHEMA,
    GET_OPTION_RETURN_VALUE_SCHEMA,
    BASE_64_SCHEMA,
} from './constants';

export async function getValue(rpcClient: JsonRpcClient, useModuleSchema: boolean, dropDown: string) {
    let entrypointName = `${CONTRACT_NAME}.get_u8`;

    switch (dropDown) {
        case 'u8':
            entrypointName = `${CONTRACT_NAME}.get_u8`;
            break;
        case 'u16':
            entrypointName = `${CONTRACT_NAME}.get_u16`;
            break;
        case 'address':
            entrypointName = `${CONTRACT_NAME}.get_address`;
            break;
        case 'contract_address':
            entrypointName = `${CONTRACT_NAME}.get_contract_address`;
            break;
        case 'account_address':
            entrypointName = `${CONTRACT_NAME}.get_account_address`;
            break;
        case 'hash':
            entrypointName = `${CONTRACT_NAME}.get_hash`;
            break;
        case 'public_key':
            entrypointName = `${CONTRACT_NAME}.get_public_key`;
            break;
        case 'signature':
            entrypointName = `${CONTRACT_NAME}.get_signature`;
            break;
        case 'timestamp':
            entrypointName = `${CONTRACT_NAME}.get_timestamp`;
            break;
        case 'string':
            entrypointName = `${CONTRACT_NAME}.get_string`;
            break;
        case 'option_u8':
            entrypointName = `${CONTRACT_NAME}.get_option_u8`;
            break;
        // We call the `get_u8` function but later use the `timestamp` schema trying to deserialize the return value.
        case 'wrong_schema':
            entrypointName = `${CONTRACT_NAME}.get_u8`;
            break;
        default:
            throw new Error(`Dropdown option does not exist`);
    }

    const res = await rpcClient.invokeContract({
        method: entrypointName,
        contract: { index: CONTRACT_INDEX, subindex: CONTRACT_SUB_INDEX },
    });

    if (!res || res.tag === 'failure' || !res.returnValue) {
        throw new Error(
            `RPC call 'invokeContract' on method '${CONTRACT_NAME}.view' of contract '${CONTRACT_INDEX}' failed`
        );
    }

    let schema = BASE_64_SCHEMA;

    switch (dropDown) {
        case 'u8':
            schema = useModuleSchema ? BASE_64_SCHEMA : GET_U8_RETURN_VALUE_SCHEMA;
            break;
        case 'u16':
            schema = useModuleSchema ? BASE_64_SCHEMA : GET_U16_RETURN_VALUE_SCHEMA;
            break;
        case 'address':
            schema = useModuleSchema ? BASE_64_SCHEMA : GET_ADDRESS_RETURN_VALUE_SCHEMA;
            break;
        case 'contract_address':
            schema = useModuleSchema ? BASE_64_SCHEMA : GET_CONTRACT_ADDRESS_RETURN_VALUE_SCHEMA;
            break;
        case 'account_address':
            schema = useModuleSchema ? BASE_64_SCHEMA : GET_ACCOUNT_ADDRESS_RETURN_VALUE_SCHEMA;
            break;
        case 'hash':
            schema = useModuleSchema ? BASE_64_SCHEMA : GET_HASH_RETURN_VALUE_SCHEMA;
            break;
        case 'public_key':
            schema = useModuleSchema ? BASE_64_SCHEMA : GET_PUBLIC_KEY_RETURN_VALUE_SCHEMA;
            break;
        case 'signature':
            schema = useModuleSchema ? BASE_64_SCHEMA : GET_SIGNATURE_RETURN_VALUE_SCHEMA;
            break;
        case 'timestamp':
            schema = useModuleSchema ? BASE_64_SCHEMA : GET_TIMESTAMP_RETURN_VALUE_SCHEMA;
            break;
        case 'string':
            schema = useModuleSchema ? BASE_64_SCHEMA : GET_STRING_RETURN_VALUE_SCHEMA;
            break;
        case 'option_u8':
            schema = useModuleSchema ? BASE_64_SCHEMA : GET_OPTION_RETURN_VALUE_SCHEMA;
            break;
        // We called the `get_u8` function but now use the `timestamp` schema trying to deserialize the return value.
        case 'wrong_schema':
            schema = useModuleSchema ? BASE_64_SCHEMA : GET_TIMESTAMP_RETURN_VALUE_SCHEMA;
            break;
        default:
            throw new Error(`Dropdown option does not exist`);
    }

    let returnValue;

    if (useModuleSchema) {
        try {
            returnValue = deserializeReceiveReturnValue(
                toBuffer(res.returnValue, 'hex'),
                toBuffer(schema, 'base64'),
                `${CONTRACT_NAME}`,
                // If dropDown === 'wrong_schema', we called the `get_u8` function but now use the `timestamp` schema trying to deserialize the return value.
                `get_${dropDown !== 'wrong_schema' ? dropDown : 'timestamp'}`
            );
        } catch (err) {
            throw new Error((err as Error).message);
        }
    } else {
        try {
            returnValue = deserializeTypeValue(toBuffer(res.returnValue, 'hex'), toBuffer(schema, 'base64'));
        } catch (err) {
            throw new Error(err as string);
        }
    }

    if (returnValue === undefined) {
        throw new Error(
            `Deserializing the returnValue from the '${CONTRACT_NAME}.${entrypointName}' method of contract '${CONTRACT_INDEX}' failed`
        );
    } else {
        return returnValue;
    }
}

export async function view(rpcClient: JsonRpcClient) {
    const res = await rpcClient.invokeContract({
        method: `${CONTRACT_NAME}.view`,
        contract: { index: CONTRACT_INDEX, subindex: CONTRACT_SUB_INDEX },
    });

    if (!res || res.tag === 'failure' || !res.returnValue) {
        throw new Error(
            `RPC call 'invokeContract' on method '${CONTRACT_NAME}.view' of contract '${CONTRACT_INDEX}' failed`
        );
    }

    const state = deserializeTypeValue(toBuffer(res.returnValue, 'hex'), toBuffer(VIEW_RETURN_VALUE_SCHEMA, 'base64'));

    if (state === undefined) {
        throw new Error(
            `Deserializing the returnValue from the '${CONTRACT_NAME}.view' method of contract '${CONTRACT_INDEX}' failed`
        );
    } else {
        return JSON.stringify(state);
    }
}

export async function accountInfo(rpcClient: JsonRpcClient, account: string) {
    return rpcClient.getAccountInfo(account);
}

export async function smartContractInfo(rpcClient: JsonRpcClient) {
    return rpcClient.getInstanceInfo({ index: CONTRACT_INDEX, subindex: CONTRACT_SUB_INDEX });
}
