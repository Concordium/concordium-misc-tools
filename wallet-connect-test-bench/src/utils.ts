import { createContext } from 'react';
import { AccountAddress, AccountTransactionType, CcdAmount, UpdateContractPayload } from '@concordium/web-sdk';
import { WalletConnection } from '@concordium/react-components';
import { typeSchemaFromBase64, moduleSchemaFromBase64, TypedSmartContractParameters } from '@concordium/wallet-connectors';
import {
    CONTRACT_NAME,
    CONTRACT_INDEX,
    CONTRACT_SUB_INDEX,
    SET_U8_PARAMETER_SCHEMA,
    BASE_64_SCHEMA,
    SET_OBJECT_PARAMETER_SCHEMA,
    SET_ADDRESS_ARRAY_PARAMETER_SCHEMA,
    SET_ACCOUNT_ADDRESS_PARAMETER_SCHEMA,
    SET_CONTRACT_ADDRESS_PARAMETER_SCHEMA,
    SET_ADDRESS_PARAMETER_SCHEMA,
    SET_U16_PARAMETER_SCHEMA,
} from './constants';

export async function set_value(connection: WalletConnection, account: string, useModuleSchema: boolean, isPayable: boolean, dropDown: string, input: string, cCDAmount: string) {

    let receiveName = `${CONTRACT_NAME}.set_u8_payable`;

    switch (dropDown) {
        case 'u8': receiveName = isPayable ? `${CONTRACT_NAME}.set_u8_payable` : `${CONTRACT_NAME}.set_u8`
            break
        case 'u16': receiveName = isPayable ? `${CONTRACT_NAME}.set_u16_payable` : `${CONTRACT_NAME}.set_u16`
            break
        case 'address': receiveName = isPayable ? `${CONTRACT_NAME}.set_address_payable` : `${CONTRACT_NAME}.set_address`
            break
        case 'contract_address': receiveName = isPayable ? `${CONTRACT_NAME}.set_contract_address_payable` : `${CONTRACT_NAME}.set_contract_address`
            break
        case 'account_address': receiveName = isPayable ? `${CONTRACT_NAME}.set_account_address_payable` : `${CONTRACT_NAME}.set_account_address`
            break;
    }

    let schema: TypedSmartContractParameters = {
        parameters: Number(7),
        schema: moduleSchemaFromBase64(BASE_64_SCHEMA)
    };

    switch (dropDown) {
        case 'u8': schema = useModuleSchema ? {
            parameters: Number(input),
            schema: moduleSchemaFromBase64(BASE_64_SCHEMA)
        } :
            {
                parameters: Number(input),
                schema: typeSchemaFromBase64(SET_U8_PARAMETER_SCHEMA)
            };
            break
        case 'u16': schema = useModuleSchema ? {
            parameters: Number(input),
            schema: moduleSchemaFromBase64(BASE_64_SCHEMA)
        } :
            {
                parameters: Number(input),
                schema: typeSchemaFromBase64(SET_U16_PARAMETER_SCHEMA)
            };
            break
        case 'address': schema = useModuleSchema ? {
            parameters: JSON.parse(input),
            schema: moduleSchemaFromBase64(BASE_64_SCHEMA)
        } :
            {
                parameters: JSON.parse(input),
                schema: typeSchemaFromBase64(SET_ADDRESS_PARAMETER_SCHEMA)
            };
            break
        case 'contract_address': schema = useModuleSchema ? {
            parameters: JSON.parse(input),
            schema: moduleSchemaFromBase64(BASE_64_SCHEMA)
        } :
            {
                parameters: JSON.parse(input),
                schema: typeSchemaFromBase64(SET_CONTRACT_ADDRESS_PARAMETER_SCHEMA)
            };
            break
        case 'account_address': schema = useModuleSchema ? {
            parameters: JSON.parse("\"" + input + "\""),
            schema: moduleSchemaFromBase64(BASE_64_SCHEMA)
        } :
            {
                parameters: JSON.parse("\"" + input + "\""),
                schema: typeSchemaFromBase64(SET_ACCOUNT_ADDRESS_PARAMETER_SCHEMA)
            };
            break;
    }

    return connection.signAndSendTransaction(
        account,
        AccountTransactionType.Update,
        {
            amount: new CcdAmount(BigInt(cCDAmount)),
            address: {
                index: CONTRACT_INDEX,
                subindex: CONTRACT_SUB_INDEX,
            },
            receiveName: receiveName,
            maxContractExecutionEnergy: 30000n,
        } as UpdateContractPayload,
        schema
    );
}

export async function set_array(connection: WalletConnection, account: string, useModuleSchema: boolean, isPayable: boolean, cCDAmount: string) {

    const inputParameter = [
        {
            "Account": [
                "4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt"
            ]
        }, {
            "Contract": [{
                "index": 3,
                "subindex": 0
            }]
        }]

    const schema = useModuleSchema ? {
        parameters: inputParameter,
        schema: moduleSchemaFromBase64(BASE_64_SCHEMA)
    } :
        {
            parameters: inputParameter,
            schema: typeSchemaFromBase64(SET_ADDRESS_ARRAY_PARAMETER_SCHEMA)
        };

    let receiveName = isPayable ? `${CONTRACT_NAME}.set_address_array_payable` : `${CONTRACT_NAME}.set_address_array`

    return connection.signAndSendTransaction(
        account,
        AccountTransactionType.Update,
        {
            amount: new CcdAmount(BigInt(cCDAmount)),
            address: {
                index: CONTRACT_INDEX,
                subindex: CONTRACT_SUB_INDEX,
            },
            receiveName: receiveName,
            maxContractExecutionEnergy: 30000n,
        } as UpdateContractPayload,
        // @ts-ignore: 
        schema
    );
}

export async function simple_CCD_transfer(connection: WalletConnection, account: string, toAccount: string, cCDAmount: string) {

    return connection.signAndSendTransaction(
        account,
        AccountTransactionType.Transfer,
        {
            amount: new CcdAmount(BigInt(cCDAmount)),
            toAddress: new AccountAddress(toAccount),
        }
    );
}

export async function simple_CCD_transfer_to_non_existing_account_address(connection: WalletConnection, account: string) {

    return connection.signAndSendTransaction(
        account,
        AccountTransactionType.Transfer,
        {
            amount: new CcdAmount(BigInt(1234n)),
            toAddress: new AccountAddress("35CJPZohio6Ztii2zy1AYzJKvuxbGG44wrBn7hLHiYLoF2nxnh"),
        }
    );
}

export async function reverts(connection: WalletConnection, account: string) {

    return connection.signAndSendTransaction(
        account,
        AccountTransactionType.Update,
        {
            amount: new CcdAmount(BigInt(0)),
            address: {
                index: CONTRACT_INDEX,
                subindex: CONTRACT_SUB_INDEX,
            },
            receiveName: `${CONTRACT_NAME}.reverts`,
            maxContractExecutionEnergy: 30000n,
        } as UpdateContractPayload
    );
}

export async function internal_call_reverts(connection: WalletConnection, account: string) {

    return connection.signAndSendTransaction(
        account,
        AccountTransactionType.Update,
        {
            amount: new CcdAmount(BigInt(0)),
            address: {
                index: CONTRACT_INDEX,
                subindex: CONTRACT_SUB_INDEX,
            },
            receiveName: `${CONTRACT_NAME}.internal_call_reverts`,
            maxContractExecutionEnergy: 30000n,
        } as UpdateContractPayload
    );
}

export async function internal_call_success(connection: WalletConnection, account: string) {

    return connection.signAndSendTransaction(
        account,
        AccountTransactionType.Update,
        {
            amount: new CcdAmount(BigInt(0)),
            address: {
                index: CONTRACT_INDEX,
                subindex: CONTRACT_SUB_INDEX,
            },
            receiveName: `${CONTRACT_NAME}.internal_call_success`,
            maxContractExecutionEnergy: 30000n,
        } as UpdateContractPayload
    );
}

export async function set_object(connection: WalletConnection, account: string, useModuleSchema: boolean, isPayable: boolean, cCDAmount: string) {

    const inputParameter = {
        "account_address_value": "4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt",
        "address_array": [
            {
                "Account": [
                    "4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt"
                ]
            }, {
                "Account": [
                    "4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt"
                ]
            }],
        "address_value": {
            "Account": [
                "4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt"
            ]
        },
        "contract_address_value": {
            "index": 3,
            "subindex": 0
        },
        "u16_value": 999,
        "u8_value": 88
    }

    const schema = useModuleSchema ? {
        parameters: inputParameter,
        schema: moduleSchemaFromBase64(BASE_64_SCHEMA)
    } :
        {
            parameters: inputParameter,
            schema: typeSchemaFromBase64(SET_OBJECT_PARAMETER_SCHEMA)
        };

    let receiveName = isPayable ? `${CONTRACT_NAME}.set_object_payable` : `${CONTRACT_NAME}.set_object`

    return connection.signAndSendTransaction(
        account,
        AccountTransactionType.Update,
        {
            amount: new CcdAmount(BigInt(cCDAmount)),
            address: {
                index: CONTRACT_INDEX,
                subindex: CONTRACT_SUB_INDEX,
            },
            receiveName: receiveName,
            maxContractExecutionEnergy: 30000n,
        } as UpdateContractPayload,
        schema
    );
}

/**
 * Global application state.
 */
export type State = {
    isConnected: boolean;
    account: string | undefined;
};

export const state = createContext<State>({ isConnected: false, account: undefined });
