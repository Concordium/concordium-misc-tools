/* eslint-disable consistent-return */
/* eslint-disable no-alert */
import { createContext } from 'react';
import { AccountTransactionType, CcdAmount, UpdateContractPayload } from '@concordium/web-sdk';
import { WalletConnection } from '@concordium/react-components';
import { typeSchemaFromBase64, moduleSchemaFromBase64, TypedSmartContractParameters } from '@concordium/wallet-connectors';
import {
    TX_CONTRACT_NAME,
    TX_CONTRACT_INDEX,
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

// /**
//  * Send update operator signature to backend.
//  */
// export async function submitUpdateOperator(backend: string, signer: string, nonce: string, signature: string, operator: string, addOperator: boolean) {

//     if (signer === '') {
//         alert('Insert an signer address.');
//         return '';
//     }

//     if (signer.length !== 50) {
//         alert('Signer address needs to have 50 digits.');
//         return '';
//     }

//     if (nonce === '') {
//         alert('Insert a nonce.');
//         return '';
//     }

//     // eslint-disable-next-line no-restricted-globals
//     if (isNaN(Number(nonce))) {
//         alert('Your nonce needs to be a number.');
//         return '';
//     }

//     if (signature === '') {
//         alert('Insert a signature.');
//         return '';
//     }

//     if (signature.length !== 128) {
//         alert('Signature needs to have 128 digits.');
//         return '';
//     }

//     if (operator === '') {
//         alert('Insert an operator address.');
//         return '';
//     }

//     if (operator.length !== 50) {
//         alert('Operator address needs to have 50 digits.');
//         return '';
//     }

//     const response = await fetch(`${backend}/submitUpdateOperator`, {
//         method: 'POST',
//         headers: new Headers({ 'content-type': 'application/json' }),
//         body: JSON.stringify({ signer, nonce: Number(nonce), signature, operator, add_operator: addOperator, timestamp: EXPIRY_TIME_SIGNATURE }),
//     });
//     if (!response.ok) {
//         const error = await response.json();
//         throw new Error('Unable to submit update operator: ' + JSON.stringify(error));
//     }
//     const body = await response.json();
//     if (body) {
//         return body;
//     }
//     throw new Error('Unable to submit update operator');
// }

// /**
//  * Send transfer signature to backend.
//  */
// export async function submitTransfer(backend: string,
//     signer: string,
//     nonce: string,
//     signature: string,
//     tokenID: string,
//     from: string,
//     to: string) {

//     if (signer === '') {
//         alert('Insert an signer address.');
//         return '';
//     }

//     if (signer.length !== 50) {
//         alert('Signer address needs to have 50 digits.');
//         return '';
//     }

//     if (nonce === '') {
//         alert('Insert a nonce.');
//         return '';
//     }

//     // eslint-disable-next-line no-restricted-globals
//     if (isNaN(Number(nonce))) {
//         alert('Your nonce needs to be a number.');
//         return '';
//     }

//     if (signature === '') {
//         alert('Insert a signature.');
//         return '';
//     }

//     if (signature.length !== 128) {
//         alert('Signature needs to have 128 digits.');
//         return '';
//     }

//     if (tokenID === '') {
//         alert('Insert a tokenID.');
//         return '';
//     }

//     if (tokenID.length !== 8) {
//         alert('TokenID needs to have 8 digits.');
//         return '';
//     }

//     if (from === '') {
//         alert('Insert a `from` address.');
//         return '';
//     }

//     if (from.length !== 50) {
//         alert('`From` address needs to have 50 digits.');
//         return '';
//     }

//     if (to === '') {
//         alert('Insert a `to` address.');
//         return '';
//     }

//     if (to.length !== 50) {
//         alert('`To` address needs to have 50 digits.');
//         return '';
//     }

//     const response = await fetch(`${backend}/submitTransfer`, {
//         method: 'POST',
//         headers: new Headers({ 'content-type': 'application/json' }),
//         body: JSON.stringify({ signer, nonce: Number(nonce), signature, token_id: tokenID, from, to, timestamp: EXPIRY_TIME_SIGNATURE }),
//     });
//     if (!response.ok) {
//         const error = await response.json();
//         throw new Error('Unable to submit transfer: ' + JSON.stringify(error));
//     }
//     const body = await response.json();
//     if (body) {
//         return body;
//     }
//     throw new Error('Unable to submit transfer');
// }

// /**
//  * Action for minting a token to the user's account.
//  */
// export async function mint(connection: WalletConnection, account: string) {
//     return connection.signAndSendTransaction(
//         account,
//         AccountTransactionType.Update,
//         {
//             amount: new CcdAmount(BigInt(0n)),
//             address: {
//                 index: SPONSORED_TX_CONTRACT_INDEX,
//                 subindex: CONTRACT_SUB_INDEX,
//             },
//             receiveName: `${SPONSORED_TX_CONTRACT_NAME}.mint`,
//             maxContractExecutionEnergy: 30000n,
//         } as UpdateContractPayload,
//         // eslint-disable-next-line @typescript-eslint/ban-ts-comment
//         // @ts-ignore
//         {
//             owner: { Account: [account] },
//         },
//         {
//             type: 'parameter',
//             value: MINT_PARAMETER_SCHEMA
//         }
//     );
// }

export async function set_value(connection: WalletConnection, account: string, useModuleSchema: boolean, isPayable: boolean, dropDown: string, input: string, cCDAmount: string) {

    let receiveName = `${TX_CONTRACT_NAME}.set_u8_payable`;

    switch (dropDown) {
        case 'u8': receiveName = isPayable ? `${TX_CONTRACT_NAME}.set_u8_payable` : `${TX_CONTRACT_NAME}.set_u8`
            break
        case 'u16': receiveName = isPayable ? `${TX_CONTRACT_NAME}.set_u16_payable` : `${TX_CONTRACT_NAME}.set_u16`
            break
        case 'Address': receiveName = isPayable ? `${TX_CONTRACT_NAME}.set_address_payable` : `${TX_CONTRACT_NAME}.set_address`
            break
        case 'ContractAddress': receiveName = isPayable ? `${TX_CONTRACT_NAME}.set_contract_address_payable` : `${TX_CONTRACT_NAME}.set_contract_address`
            break
        case 'AccountAddress': receiveName = isPayable ? `${TX_CONTRACT_NAME}.set_account_address_payable` : `${TX_CONTRACT_NAME}.set_account_address`
            break;
    }

    let schema: TypedSmartContractParameters = {
        parameters: Number(7),
        schema: moduleSchemaFromBase64(BASE_64_SCHEMA)
    };

    console.log(schema)

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
        case 'Address': schema = useModuleSchema ? {
            parameters: JSON.parse(input),
            schema: moduleSchemaFromBase64(BASE_64_SCHEMA)
        } :
            {
                parameters: JSON.parse(input),
                schema: typeSchemaFromBase64(SET_ADDRESS_PARAMETER_SCHEMA)
            };
            break
        case 'ContractAddress': schema = useModuleSchema ? {
            parameters: JSON.parse(input),
            schema: moduleSchemaFromBase64(BASE_64_SCHEMA)
        } :
            {
                parameters: JSON.parse(input),
                schema: typeSchemaFromBase64(SET_CONTRACT_ADDRESS_PARAMETER_SCHEMA)
            };
            break
        case 'AccountAddress': schema = useModuleSchema ? {
            parameters: JSON.parse("\"" + input + "\""),
            schema: moduleSchemaFromBase64(BASE_64_SCHEMA)
        } :
            {
                parameters: JSON.parse("\"" + input + "\""),
                schema: typeSchemaFromBase64(SET_ACCOUNT_ADDRESS_PARAMETER_SCHEMA)
            };
            break;
    }

    console.log(schema)

    return connection.signAndSendTransaction(
        account,
        AccountTransactionType.Update,
        {
            amount: new CcdAmount(BigInt(cCDAmount)),
            address: {
                index: TX_CONTRACT_INDEX,
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

    let receiveName = isPayable ? `${TX_CONTRACT_NAME}.set_address_array_payable` : `${TX_CONTRACT_NAME}.set_address_array`

    return connection.signAndSendTransaction(
        account,
        AccountTransactionType.Update,
        {
            amount: new CcdAmount(BigInt(cCDAmount)),
            address: {
                index: TX_CONTRACT_INDEX,
                subindex: CONTRACT_SUB_INDEX,
            },
            receiveName: receiveName,
            maxContractExecutionEnergy: 30000n,
        } as UpdateContractPayload,
        schema
    );
}

export async function reverts(connection: WalletConnection, account: string) {

    return connection.signAndSendTransaction(
        account,
        AccountTransactionType.Update,
        {
            amount: new CcdAmount(BigInt(0)),
            address: {
                index: TX_CONTRACT_INDEX,
                subindex: CONTRACT_SUB_INDEX,
            },
            receiveName: `${TX_CONTRACT_NAME}.reverts`,
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
                index: TX_CONTRACT_INDEX,
                subindex: CONTRACT_SUB_INDEX,
            },
            receiveName: `${TX_CONTRACT_NAME}.internal_call_reverts`,
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
                index: TX_CONTRACT_INDEX,
                subindex: CONTRACT_SUB_INDEX,
            },
            receiveName: `${TX_CONTRACT_NAME}.internal_call_success`,
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

    let receiveName = isPayable ? `${TX_CONTRACT_NAME}.set_object_payable` : `${TX_CONTRACT_NAME}.set_object`

    return connection.signAndSendTransaction(
        account,
        AccountTransactionType.Update,
        {
            amount: new CcdAmount(BigInt(cCDAmount)),
            address: {
                index: TX_CONTRACT_INDEX,
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
