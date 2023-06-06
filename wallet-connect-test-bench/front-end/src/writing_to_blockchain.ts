import { createContext } from 'react';
import {
    SchemaType,
    SchemaWithContext,
    SmartContractParameters,
    WalletApi,
} from '@concordium/browser-wallet-api-helpers';
import {
    AccountAddress,
    AccountTransactionType,
    CcdAmount,
    DeployModulePayload,
    InitContractPayload,
    ModuleReference,
    UpdateContractPayload,
    toBuffer,
} from '@concordium/web-sdk';
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
    SET_HASH_PARAMETER_SCHEMA,
    SET_PUBLIC_KEY_PARAMETER_SCHEMA,
    SET_SIGNATURE_PARAMETER_SCHEMA,
    SET_TIMESTAMP_PARAMETER_SCHEMA,
    SET_STRING_PARAMETER_SCHEMA,
    SET_OPTION_PARAMETER_SCHEMA,
    BASE_64_TEST_BENCH_SMART_CONTRACT_MODULE,
} from './constants';

export async function initializeWithoutAmountWithoutParameter(connection: WalletApi, account: string) {
    return connection.sendTransaction(account, AccountTransactionType.InitContract, {
        amount: new CcdAmount(BigInt(0)),
        moduleRef: new ModuleReference('4f013778fc2ab2136d12ae994303bcc941619a16f6c80f22e189231781c087c7'),
        initName: 'smart_contract_test_bench',
        param: toBuffer(''),
        maxContractExecutionEnergy: 30000n,
    } as InitContractPayload);
}

export async function initializeWithAmount(connection: WalletApi, account: string, cCDAmount: string) {
    return connection.sendTransaction(account, AccountTransactionType.InitContract, {
        amount: new CcdAmount(BigInt(cCDAmount)),
        moduleRef: new ModuleReference('4f013778fc2ab2136d12ae994303bcc941619a16f6c80f22e189231781c087c7'),
        initName: 'smart_contract_test_bench',
        param: toBuffer(''),
        maxContractExecutionEnergy: 30000n,
    } as InitContractPayload);
}

export async function initializeWithParameter(
    connection: WalletApi,
    account: string,
    useModuleSchema: boolean,
    input: string
) {
    const schema = useModuleSchema
        ? {
              type: SchemaType.Module,
              value: BASE_64_SCHEMA.toString(),
          }
        : {
              type: SchemaType.Parameter,
              value: SET_U16_PARAMETER_SCHEMA.toString(),
          };

    return connection.sendTransaction(
        account,
        AccountTransactionType.InitContract,
        {
            amount: new CcdAmount(BigInt(0)),
            moduleRef: new ModuleReference('4f013778fc2ab2136d12ae994303bcc941619a16f6c80f22e189231781c087c7'),
            initName: 'smart_contract_test_bench',
            param: toBuffer(''),
            maxContractExecutionEnergy: 30000n,
        } as InitContractPayload,
        Number(input),
        schema
    );
}

export async function deploy(connection: WalletApi, account: string) {
    return connection.sendTransaction(account, AccountTransactionType.DeployModule, {
        source: toBuffer(BASE_64_TEST_BENCH_SMART_CONTRACT_MODULE, 'base64'),
    } as DeployModulePayload);
}

export async function setValue(
    connection: WalletApi,
    account: string,
    useModuleSchema: boolean,
    isPayable: boolean,
    dropDown: string,
    input: string,
    cCDAmount: string
) {
    let receiveName = `${CONTRACT_NAME}.set_u8_payable`;

    switch (dropDown) {
        case 'u8':
            receiveName = isPayable ? `${CONTRACT_NAME}.set_u8_payable` : `${CONTRACT_NAME}.set_u8`;
            break;
        case 'u16':
            receiveName = isPayable ? `${CONTRACT_NAME}.set_u16_payable` : `${CONTRACT_NAME}.set_u16`;
            break;
        case 'address':
            receiveName = isPayable ? `${CONTRACT_NAME}.set_address_payable` : `${CONTRACT_NAME}.set_address`;
            break;
        case 'contract_address':
            receiveName = isPayable
                ? `${CONTRACT_NAME}.set_contract_address_payable`
                : `${CONTRACT_NAME}.set_contract_address`;
            break;
        case 'account_address':
            receiveName = isPayable
                ? `${CONTRACT_NAME}.set_account_address_payable`
                : `${CONTRACT_NAME}.set_account_address`;
            break;
        case 'hash':
            receiveName = isPayable ? `${CONTRACT_NAME}.set_hash_payable` : `${CONTRACT_NAME}.set_hash`;
            break;
        case 'public_key':
            receiveName = isPayable ? `${CONTRACT_NAME}.set_public_key_payable` : `${CONTRACT_NAME}.set_public_key`;
            break;
        case 'signature':
            receiveName = isPayable ? `${CONTRACT_NAME}.set_signature_payable` : `${CONTRACT_NAME}.set_signature`;
            break;
        case 'timestamp':
            receiveName = isPayable ? `${CONTRACT_NAME}.set_timestamp_payable` : `${CONTRACT_NAME}.set_timestamp`;
            break;
        case 'string':
            receiveName = isPayable ? `${CONTRACT_NAME}.set_string_payable` : `${CONTRACT_NAME}.set_string`;
            break;
        case 'option_u8_none':
            receiveName = isPayable ? `${CONTRACT_NAME}.set_option_u8_payable` : `${CONTRACT_NAME}.set_option_u8`;
            break;
        case 'option_u8_some':
            receiveName = isPayable ? `${CONTRACT_NAME}.set_option_u8_payable` : `${CONTRACT_NAME}.set_option_u8`;
            break;
        // We try to call the `set_u8` function but input a string as the input parameter.
        case 'wrong_schema':
            receiveName = isPayable ? `${CONTRACT_NAME}.set_u8_payable` : `${CONTRACT_NAME}.set_u8`;
            break;
        default:
            throw new Error(`Dropdown option does not exist`);
    }

    let schema: SchemaWithContext = {
        type: SchemaType.Module,
        value: BASE_64_SCHEMA.toString(),
    };

    switch (dropDown) {
        case 'u8':
            schema = useModuleSchema
                ? {
                      type: SchemaType.Module,
                      value: BASE_64_SCHEMA.toString(),
                  }
                : {
                      type: SchemaType.Parameter,
                      value: SET_U8_PARAMETER_SCHEMA.toString(),
                  };
            break;
        case 'u16':
            schema = useModuleSchema
                ? {
                      type: SchemaType.Module,
                      value: BASE_64_SCHEMA.toString(),
                  }
                : {
                      type: SchemaType.Parameter,
                      value: SET_U16_PARAMETER_SCHEMA.toString(),
                  };
            break;
        case 'address':
            schema = useModuleSchema
                ? {
                      type: SchemaType.Module,
                      value: BASE_64_SCHEMA.toString(),
                  }
                : {
                      type: SchemaType.Parameter,
                      value: SET_ADDRESS_PARAMETER_SCHEMA.toString(),
                  };

            break;
        case 'contract_address':
            schema = useModuleSchema
                ? {
                      type: SchemaType.Module,
                      value: BASE_64_SCHEMA.toString(),
                  }
                : {
                      type: SchemaType.Parameter,
                      value: SET_CONTRACT_ADDRESS_PARAMETER_SCHEMA.toString(),
                  };
            break;
        case 'account_address':
            schema = useModuleSchema
                ? {
                      type: SchemaType.Module,
                      value: BASE_64_SCHEMA.toString(),
                  }
                : {
                      type: SchemaType.Parameter,
                      value: SET_ACCOUNT_ADDRESS_PARAMETER_SCHEMA.toString(),
                  };
            break;
        case 'hash':
            schema = useModuleSchema
                ? {
                      type: SchemaType.Module,
                      value: BASE_64_SCHEMA.toString(),
                  }
                : {
                      type: SchemaType.Parameter,
                      value: SET_HASH_PARAMETER_SCHEMA.toString(),
                  };

            break;
        case 'public_key':
            schema = useModuleSchema
                ? {
                      type: SchemaType.Module,
                      value: BASE_64_SCHEMA.toString(),
                  }
                : {
                      type: SchemaType.Parameter,
                      value: SET_PUBLIC_KEY_PARAMETER_SCHEMA.toString(),
                  };

            break;
        case 'signature':
            schema = useModuleSchema
                ? {
                      type: SchemaType.Module,
                      value: BASE_64_SCHEMA.toString(),
                  }
                : {
                      type: SchemaType.Parameter,
                      value: SET_SIGNATURE_PARAMETER_SCHEMA.toString(),
                  };
            break;
        case 'timestamp':
            schema = useModuleSchema
                ? {
                      type: SchemaType.Module,
                      value: BASE_64_SCHEMA.toString(),
                  }
                : {
                      type: SchemaType.Parameter,
                      value: SET_TIMESTAMP_PARAMETER_SCHEMA.toString(),
                  };

            break;
        case 'string':
            schema = useModuleSchema
                ? {
                      type: SchemaType.Module,
                      value: BASE_64_SCHEMA.toString(),
                  }
                : {
                      type: SchemaType.Parameter,
                      value: SET_STRING_PARAMETER_SCHEMA.toString(),
                  };

            break;
        case 'option_u8_none':
            schema = useModuleSchema
                ? {
                      type: SchemaType.Module,
                      value: BASE_64_SCHEMA.toString(),
                  }
                : {
                      type: SchemaType.Parameter,
                      value: SET_OPTION_PARAMETER_SCHEMA.toString(),
                  };

            break;
        case 'option_u8_some':
            schema = useModuleSchema
                ? {
                      type: SchemaType.Module,
                      value: BASE_64_SCHEMA.toString(),
                  }
                : {
                      type: SchemaType.Parameter,
                      value: SET_OPTION_PARAMETER_SCHEMA.toString(),
                  };

            break;
        // We called the `set_u8` function but input a string now as the input parameter.
        case 'wrong_schema':
            schema = useModuleSchema
                ? {
                      type: SchemaType.Module,
                      value: BASE_64_SCHEMA.toString(),
                  }
                : {
                      type: SchemaType.Parameter,
                      value: SET_U8_PARAMETER_SCHEMA.toString(),
                  };

            break;
        default:
            throw new Error(`Dropdown option does not exist`);
    }

    let parameters;

    switch (dropDown) {
        case 'u8':
            parameters = Number(input);
            break;
        case 'u16':
            parameters = Number(input);
            break;
        case 'address':
            parameters = JSON.parse(input);
            break;
        case 'contract_address':
            parameters = JSON.parse(input);
            break;
        case 'account_address':
            parameters = JSON.parse(`"${input}"`);
            break;
        case 'hash':
            parameters = JSON.parse(`"${input}"`);
            break;
        case 'public_key':
            parameters = JSON.parse(`"${input}"`);
            break;
        case 'signature':
            parameters = JSON.parse(`"${input}"`);
            break;
        case 'timestamp':
            parameters = JSON.parse(`"${input}"`);
            break;
        case 'string':
            parameters = JSON.parse(`"${input}"`);
            break;
        case 'option_u8_none':
            parameters = { None: [] };
            break;
        case 'option_u8_some':
            parameters = { Some: [Number(input)] };
            break;
        // We called the `set_u8` function but input a string now as the input parameter.
        case 'wrong_schema':
            parameters = 'wrong input parameter type';
            break;
        default:
            throw new Error(`Dropdown option does not exist`);
    }

    return connection.sendTransaction(
        account,
        AccountTransactionType.Update,
        {
            amount: new CcdAmount(BigInt(cCDAmount)),
            address: {
                index: CONTRACT_INDEX,
                subindex: CONTRACT_SUB_INDEX,
            },
            receiveName,
            maxContractExecutionEnergy: 30000n,
        },
        parameters,
        schema
    );
}

export async function setArray(
    connection: WalletApi,
    account: string,
    useModuleSchema: boolean,
    isPayable: boolean,
    cCDAmount: string
) {
    const inputParameter = [
        {
            Account: ['4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt'],
        },
        {
            Contract: [
                {
                    index: 3,
                    subindex: 0,
                },
            ],
        },
    ] as SmartContractParameters;

    const schema = useModuleSchema
        ? {
              type: SchemaType.Module,
              value: BASE_64_SCHEMA.toString(),
          }
        : {
              type: SchemaType.Parameter,
              value: SET_ADDRESS_ARRAY_PARAMETER_SCHEMA.toString(),
          };

    const receiveName = isPayable ? `${CONTRACT_NAME}.set_address_array_payable` : `${CONTRACT_NAME}.set_address_array`;

    return connection.sendTransaction(
        account,
        AccountTransactionType.Update,
        {
            amount: new CcdAmount(BigInt(cCDAmount)),
            address: {
                index: CONTRACT_INDEX,
                subindex: CONTRACT_SUB_INDEX,
            },
            receiveName,
            maxContractExecutionEnergy: 30000n,
        } as UpdateContractPayload,
        inputParameter,
        schema
    );
}

export async function setObject(
    connection: WalletApi,
    account: string,
    useModuleSchema: boolean,
    isPayable: boolean,
    cCDAmount: string
) {
    const inputParameter = {
        account_address_value: '4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt',
        address_array: [
            {
                Account: ['4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt'],
            },
            {
                Account: ['4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt'],
            },
        ],
        address_value: {
            Account: ['4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt'],
        },
        contract_address_value: {
            index: 3,
            subindex: 0,
        },
        u16_value: 999,
        u8_value: 88,
        hash_value: '37a2a8e52efad975dbf6580e7734e4f249eaa5ea8a763e934a8671cd7e446499',
        option_value: {
            None: [],
        },
        public_key_value: '37a2a8e52efad975dbf6580e7734e4f249eaa5ea8a763e934a8671cd7e446499',
        signature_value:
            '632f567c9321405ce201a0a38615da41efe259ede154ff45ad96cdf860718e79bde07cff72c4d119c644552a8c7f0c413f5cf5390b0ea0458993d6d6374bd904',
        string_value: 'abc',
        timestamp_value: '2030-08-08T05:15:00Z',
    };

    const schema = useModuleSchema
        ? {
              type: SchemaType.Module,
              value: BASE_64_SCHEMA.toString(),
          }
        : {
              type: SchemaType.Parameter,
              value: SET_OBJECT_PARAMETER_SCHEMA.toString(),
          };

    const receiveName = isPayable ? `${CONTRACT_NAME}.set_object_payable` : `${CONTRACT_NAME}.set_object`;

    return connection.sendTransaction(
        account,
        AccountTransactionType.Update,
        {
            amount: new CcdAmount(BigInt(cCDAmount)),
            address: {
                index: CONTRACT_INDEX,
                subindex: CONTRACT_SUB_INDEX,
            },
            receiveName,
            maxContractExecutionEnergy: 30000n,
        } as UpdateContractPayload,
        inputParameter,
        schema
    );
}

export async function simpleCCDTransfer(connection: WalletApi, account: string, toAccount: string, cCDAmount: string) {
    return connection.sendTransaction(account, AccountTransactionType.Transfer, {
        amount: new CcdAmount(BigInt(cCDAmount)),
        toAddress: new AccountAddress(toAccount),
    });
}

export async function simpleCCDTransferToNonExistingAccountAddress(connection: WalletApi, account: string) {
    return connection.sendTransaction(account, AccountTransactionType.Transfer, {
        amount: new CcdAmount(BigInt(1n)),
        toAddress: new AccountAddress('35CJPZohio6Ztii2zy1AYzJKvuxbGG44wrBn7hLHiYLoF2nxnh'),
    });
}

export async function reverts(connection: WalletApi, account: string) {
    return connection.sendTransaction(account, AccountTransactionType.Update, {
        amount: new CcdAmount(BigInt(0)),
        address: {
            index: CONTRACT_INDEX,
            subindex: CONTRACT_SUB_INDEX,
        },
        receiveName: `${CONTRACT_NAME}.reverts`,
        maxContractExecutionEnergy: 30000n,
    } as UpdateContractPayload);
}

export async function internalCallReverts(connection: WalletApi, account: string) {
    return connection.sendTransaction(account, AccountTransactionType.Update, {
        amount: new CcdAmount(BigInt(0)),
        address: {
            index: CONTRACT_INDEX,
            subindex: CONTRACT_SUB_INDEX,
        },
        receiveName: `${CONTRACT_NAME}.internal_call_reverts`,
        maxContractExecutionEnergy: 30000n,
    } as UpdateContractPayload);
}

export async function internalCallSuccess(connection: WalletApi, account: string) {
    return connection.sendTransaction(account, AccountTransactionType.Update, {
        amount: new CcdAmount(BigInt(0)),
        address: {
            index: CONTRACT_INDEX,
            subindex: CONTRACT_SUB_INDEX,
        },
        receiveName: `${CONTRACT_NAME}.internal_call_success`,
        maxContractExecutionEnergy: 30000n,
    } as UpdateContractPayload);
}

export async function notExistingEntrypoint(connection: WalletApi, account: string) {
    return connection.sendTransaction(
        account,
        AccountTransactionType.Update,
        {
            amount: new CcdAmount(BigInt(0)),
            address: {
                index: CONTRACT_INDEX,
                subindex: CONTRACT_SUB_INDEX,
            },
            receiveName: `${CONTRACT_NAME}.does_not_exist`,
            maxContractExecutionEnergy: 30000n,
        } as UpdateContractPayload,
        3,
        {
            type: SchemaType.Parameter,
            value: SET_U8_PARAMETER_SCHEMA.toString(),
        }
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
