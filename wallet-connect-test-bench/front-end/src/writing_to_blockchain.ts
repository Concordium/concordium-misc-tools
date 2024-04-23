import { createContext } from "react";
import { SmartContractParameters } from "@concordium/browser-wallet-api-helpers";
import {
  AccountAddress,
  AccountTransactionType,
  CcdAmount,
  ModuleReference,
  toBuffer,
  ReceiveName,
  Energy,
  ContractName,
  ContractAddress,
} from "@concordium/web-sdk";
import { WalletConnection } from "@concordium/react-components";
import {
  typeSchemaFromBase64,
  moduleSchemaFromBase64,
  TypedSmartContractParameters,
} from "@concordium/wallet-connectors";
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
} from "./constants";

export async function initializeWithoutAmountWithoutParameter(
  connection: WalletConnection,
  account: string
) {
  const payload = {
    amount: CcdAmount.fromMicroCcd(0),
    moduleRef: ModuleReference.fromHexString(
      "4f013778fc2ab2136d12ae994303bcc941619a16f6c80f22e189231781c087c7"
    ),
    initName: ContractName.fromString("smart_contract_test_bench"),
    param: toBuffer(""),
    maxContractExecutionEnergy: Energy.create(30000n),
  };

  console.debug("Sending init transaction:");
  console.debug("Payload:");
  console.debug(payload);
  console.debug("Account:");
  console.debug(account);
  console.debug("");

  return connection.signAndSendTransaction(
    account,
    AccountTransactionType.InitContract,
    payload
  );
}

export async function initializeWithAmount(
  connection: WalletConnection,
  account: string,
  cCDAmount: string
) {
  const payload = {
    amount: CcdAmount.fromMicroCcd(cCDAmount ? cCDAmount : 0),
    moduleRef: ModuleReference.fromHexString(
      "4f013778fc2ab2136d12ae994303bcc941619a16f6c80f22e189231781c087c7"
    ),
    initName: ContractName.fromString("smart_contract_test_bench"),
    param: toBuffer(""),
    maxContractExecutionEnergy: Energy.create(30000n),
  };

  console.debug("Sending init transaction:");
  console.debug("Payload:");
  console.debug(payload);
  console.debug("Account:");
  console.debug(account);
  console.debug("");

  return connection.signAndSendTransaction(
    account,
    AccountTransactionType.InitContract,
    payload
  );
}

export async function initializeWithParameter(
  connection: WalletConnection,
  account: string,
  useModuleSchema: boolean,
  input: string
) {
  const schema = useModuleSchema
    ? {
        parameters: Number(input),
        schema: moduleSchemaFromBase64(BASE_64_SCHEMA),
      }
    : {
        parameters: Number(input),
        schema: typeSchemaFromBase64(SET_U16_PARAMETER_SCHEMA),
      };

  const payload = {
    amount: CcdAmount.fromMicroCcd(0),
    moduleRef: ModuleReference.fromHexString(
      "4f013778fc2ab2136d12ae994303bcc941619a16f6c80f22e189231781c087c7"
    ),
    initName: ContractName.fromString("smart_contract_test_bench"),
    param: toBuffer(""),
    maxContractExecutionEnergy: Energy.create(30000n),
  };

  console.debug("Sending init transaction:");
  console.debug("Schema:");
  console.debug(schema);
  console.debug("Payload:");
  console.debug(payload);
  console.debug("Account:");
  console.debug(account);
  console.debug("");

  return connection.signAndSendTransaction(
    account,
    AccountTransactionType.InitContract,
    payload,
    schema
  );
}

export async function deploy(connection: WalletConnection, account: string) {
  const payload = {
    source: toBuffer(BASE_64_TEST_BENCH_SMART_CONTRACT_MODULE, "base64"),
  };

  console.debug("Sending deploy transaction:");
  console.debug("Payload:");
  console.debug(payload);
  console.debug("Account:");
  console.debug(account);
  console.debug("");

  return connection.signAndSendTransaction(
    account,
    AccountTransactionType.DeployModule,
    payload
  );
}

export async function setValue(
  connection: WalletConnection,
  account: string,
  useModuleSchema: boolean,
  isPayable: boolean,
  dropDown: string,
  input: string,
  cCDAmount: string,
  maxEnergy: string
) {
  let receiveName = ReceiveName.fromString(`${CONTRACT_NAME}.set_u8_payable`);

  switch (dropDown) {
    case "u8":
      receiveName = ReceiveName.fromString(
        isPayable
          ? `${CONTRACT_NAME}.set_u8_payable`
          : `${CONTRACT_NAME}.set_u8`
      );
      break;
    case "u16":
      receiveName = ReceiveName.fromString(
        isPayable
          ? `${CONTRACT_NAME}.set_u16_payable`
          : `${CONTRACT_NAME}.set_u16`
      );
      break;
    case "address":
      receiveName = ReceiveName.fromString(
        isPayable
          ? `${CONTRACT_NAME}.set_address_payable`
          : `${CONTRACT_NAME}.set_address`
      );
      break;
    case "contract_address":
      receiveName = ReceiveName.fromString(
        isPayable
          ? `${CONTRACT_NAME}.set_contract_address_payable`
          : `${CONTRACT_NAME}.set_contract_address`
      );
      break;
    case "account_address":
      receiveName = ReceiveName.fromString(
        isPayable
          ? `${CONTRACT_NAME}.set_account_address_payable`
          : `${CONTRACT_NAME}.set_account_address`
      );
      break;
    case "hash":
      receiveName = ReceiveName.fromString(
        isPayable
          ? `${CONTRACT_NAME}.set_hash_payable`
          : `${CONTRACT_NAME}.set_hash`
      );
      break;
    case "public_key":
      receiveName = ReceiveName.fromString(
        isPayable
          ? `${CONTRACT_NAME}.set_public_key_payable`
          : `${CONTRACT_NAME}.set_public_key`
      );
      break;
    case "signature":
      receiveName = ReceiveName.fromString(
        isPayable
          ? `${CONTRACT_NAME}.set_signature_payable`
          : `${CONTRACT_NAME}.set_signature`
      );
      break;
    case "timestamp":
      receiveName = ReceiveName.fromString(
        isPayable
          ? `${CONTRACT_NAME}.set_timestamp_payable`
          : `${CONTRACT_NAME}.set_timestamp`
      );
      break;
    case "string":
      receiveName = ReceiveName.fromString(
        isPayable
          ? `${CONTRACT_NAME}.set_string_payable`
          : `${CONTRACT_NAME}.set_string`
      );
      break;
    case "option_u8_none":
      receiveName = ReceiveName.fromString(
        isPayable
          ? `${CONTRACT_NAME}.set_option_u8_payable`
          : `${CONTRACT_NAME}.set_option_u8`
      );
      break;
    case "option_u8_some":
      receiveName = ReceiveName.fromString(
        isPayable
          ? `${CONTRACT_NAME}.set_option_u8_payable`
          : `${CONTRACT_NAME}.set_option_u8`
      );
      break;
    // We try to call the `set_u8` function but input a string as the input parameter.
    case "wrong_schema":
      receiveName = ReceiveName.fromString(
        isPayable
          ? `${CONTRACT_NAME}.set_u8_payable`
          : `${CONTRACT_NAME}.set_u8`
      );
      break;
    default:
      throw new Error(`Dropdown option does not exist`);
  }

  let schema: TypedSmartContractParameters = {
    parameters: Number(7),
    schema: moduleSchemaFromBase64(BASE_64_SCHEMA),
  };

  switch (dropDown) {
    case "u8":
      schema = useModuleSchema
        ? {
            parameters: Number(input),
            schema: moduleSchemaFromBase64(BASE_64_SCHEMA),
          }
        : {
            parameters: Number(input),
            schema: typeSchemaFromBase64(SET_U8_PARAMETER_SCHEMA),
          };
      break;
    case "u16":
      schema = useModuleSchema
        ? {
            parameters: Number(input),
            schema: moduleSchemaFromBase64(BASE_64_SCHEMA),
          }
        : {
            parameters: Number(input),
            schema: typeSchemaFromBase64(SET_U16_PARAMETER_SCHEMA),
          };
      break;
    case "address":
      schema = useModuleSchema
        ? {
            parameters: JSON.parse(input),
            schema: moduleSchemaFromBase64(BASE_64_SCHEMA),
          }
        : {
            parameters: JSON.parse(input),
            schema: typeSchemaFromBase64(SET_ADDRESS_PARAMETER_SCHEMA),
          };
      break;
    case "contract_address":
      schema = useModuleSchema
        ? {
            parameters: JSON.parse(input),
            schema: moduleSchemaFromBase64(BASE_64_SCHEMA),
          }
        : {
            parameters: JSON.parse(input),
            schema: typeSchemaFromBase64(SET_CONTRACT_ADDRESS_PARAMETER_SCHEMA),
          };
      break;
    case "account_address":
      schema = useModuleSchema
        ? {
            parameters: JSON.parse(`"${input}"`),
            schema: moduleSchemaFromBase64(BASE_64_SCHEMA),
          }
        : {
            parameters: JSON.parse(`"${input}"`),
            schema: typeSchemaFromBase64(SET_ACCOUNT_ADDRESS_PARAMETER_SCHEMA),
          };
      break;
    case "hash":
      schema = useModuleSchema
        ? {
            parameters: JSON.parse(`"${input}"`),
            schema: moduleSchemaFromBase64(BASE_64_SCHEMA),
          }
        : {
            parameters: JSON.parse(`"${input}"`),
            schema: typeSchemaFromBase64(SET_HASH_PARAMETER_SCHEMA),
          };
      break;
    case "public_key":
      schema = useModuleSchema
        ? {
            parameters: JSON.parse(`"${input}"`),
            schema: moduleSchemaFromBase64(BASE_64_SCHEMA),
          }
        : {
            parameters: JSON.parse(`"${input}"`),
            schema: typeSchemaFromBase64(SET_PUBLIC_KEY_PARAMETER_SCHEMA),
          };
      break;
    case "signature":
      schema = useModuleSchema
        ? {
            parameters: JSON.parse(`"${input}"`),
            schema: moduleSchemaFromBase64(BASE_64_SCHEMA),
          }
        : {
            parameters: JSON.parse(`"${input}"`),
            schema: typeSchemaFromBase64(SET_SIGNATURE_PARAMETER_SCHEMA),
          };
      break;
    case "timestamp":
      schema = useModuleSchema
        ? {
            parameters: JSON.parse(`"${input}"`),
            schema: moduleSchemaFromBase64(BASE_64_SCHEMA),
          }
        : {
            parameters: JSON.parse(`"${input}"`),
            schema: typeSchemaFromBase64(SET_TIMESTAMP_PARAMETER_SCHEMA),
          };
      break;
    case "string":
      schema = useModuleSchema
        ? {
            parameters: JSON.parse(`"${input}"`),
            schema: moduleSchemaFromBase64(BASE_64_SCHEMA),
          }
        : {
            parameters: JSON.parse(`"${input}"`),
            schema: typeSchemaFromBase64(SET_STRING_PARAMETER_SCHEMA),
          };
      break;
    case "option_u8_none":
      schema = useModuleSchema
        ? {
            parameters: { None: [] },
            schema: moduleSchemaFromBase64(BASE_64_SCHEMA),
          }
        : {
            parameters: { None: [] },
            schema: typeSchemaFromBase64(SET_OPTION_PARAMETER_SCHEMA),
          };
      break;
    case "option_u8_some":
      schema = useModuleSchema
        ? {
            parameters: { Some: [Number(input)] },
            schema: moduleSchemaFromBase64(BASE_64_SCHEMA),
          }
        : {
            parameters: { Some: [Number(input)] },
            schema: typeSchemaFromBase64(SET_OPTION_PARAMETER_SCHEMA),
          };
      break;
    // We called the `set_u8` function but input a string now as the input parameter.
    case "wrong_schema":
      schema = useModuleSchema
        ? {
            parameters: "wrong input parameter type",
            schema: moduleSchemaFromBase64(BASE_64_SCHEMA),
          }
        : {
            parameters: "wrong input parameter type",
            schema: typeSchemaFromBase64(SET_U8_PARAMETER_SCHEMA),
          };
      break;
    default:
      throw new Error(`Dropdown option does not exist`);
  }

  const payload = {
    amount: CcdAmount.fromMicroCcd(cCDAmount ? cCDAmount : 0),
    address: ContractAddress.create(CONTRACT_INDEX, CONTRACT_SUB_INDEX),
    receiveName,
    maxContractExecutionEnergy: Energy.create(
      BigInt(maxEnergy ? maxEnergy : 30000)
    ),
  };

  console.debug("Sending update transaction:");
  console.debug("Schema:");
  console.debug(schema);
  console.debug("Payload:");
  console.debug(payload);
  console.debug("Account:");
  console.debug(account);
  console.debug("");

  return connection.signAndSendTransaction(
    account,
    AccountTransactionType.Update,
    payload,
    schema
  );
}

export async function setArray(
  connection: WalletConnection,
  account: string,
  useModuleSchema: boolean,
  isPayable: boolean,
  cCDAmount: string
) {
  const inputParameter = [
    {
      Account: ["4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt"],
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
        parameters: inputParameter,
        schema: moduleSchemaFromBase64(BASE_64_SCHEMA),
      }
    : {
        parameters: inputParameter,
        schema: typeSchemaFromBase64(SET_ADDRESS_ARRAY_PARAMETER_SCHEMA),
      };

  const receiveName = ReceiveName.fromString(
    isPayable
      ? `${CONTRACT_NAME}.set_address_array_payable`
      : `${CONTRACT_NAME}.set_address_array`
  );

  const payload = {
    amount: CcdAmount.fromMicroCcd(cCDAmount ? cCDAmount : 0),
    address: ContractAddress.create(CONTRACT_INDEX, CONTRACT_SUB_INDEX),
    receiveName,
    maxContractExecutionEnergy: Energy.create(30000n),
  };

  console.debug("Sending update transaction:");
  console.debug("Schema:");
  console.debug(schema);
  console.debug("Payload:");
  console.debug(payload);
  console.debug("Account:");
  console.debug(account);
  console.debug("");

  return connection.signAndSendTransaction(
    account,
    AccountTransactionType.Update,
    payload,
    schema
  );
}

export async function setObject(
  connection: WalletConnection,
  account: string,
  useModuleSchema: boolean,
  isPayable: boolean,
  cCDAmount: string
) {
  const inputParameter = {
    account_address_value: "4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt",
    address_array: [
      {
        Account: ["4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt"],
      },
      {
        Account: ["4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt"],
      },
    ],
    address_value: {
      Account: ["4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt"],
    },
    contract_address_value: {
      index: 3,
      subindex: 0,
    },
    u16_value: 999,
    u8_value: 88,
    hash_value:
      "37a2a8e52efad975dbf6580e7734e4f249eaa5ea8a763e934a8671cd7e446499",
    option_value: {
      None: [],
    },
    public_key_value:
      "37a2a8e52efad975dbf6580e7734e4f249eaa5ea8a763e934a8671cd7e446499",
    signature_value:
      "632f567c9321405ce201a0a38615da41efe259ede154ff45ad96cdf860718e79bde07cff72c4d119c644552a8c7f0c413f5cf5390b0ea0458993d6d6374bd904",
    string_value: "abc",
    timestamp_value: "2030-08-08T05:15:00Z",
  };

  const schema = useModuleSchema
    ? {
        parameters: inputParameter,
        schema: moduleSchemaFromBase64(BASE_64_SCHEMA),
      }
    : {
        parameters: inputParameter,
        schema: typeSchemaFromBase64(SET_OBJECT_PARAMETER_SCHEMA),
      };

  const receiveName = ReceiveName.fromString(
    isPayable
      ? `${CONTRACT_NAME}.set_object_payable`
      : `${CONTRACT_NAME}.set_object`
  );

  const payload = {
    amount: CcdAmount.fromMicroCcd(cCDAmount ? cCDAmount : 0),
    address: ContractAddress.create(CONTRACT_INDEX, CONTRACT_SUB_INDEX),
    receiveName,
    maxContractExecutionEnergy: Energy.create(30000n),
  };

  console.debug("Sending update transaction:");
  console.debug("Schema:");
  console.debug(schema);
  console.debug("Payload:");
  console.debug(payload);
  console.debug("Account:");
  console.debug(account);
  console.debug("");

  return connection.signAndSendTransaction(
    account,
    AccountTransactionType.Update,
    payload,
    schema
  );
}

export async function simpleCCDTransfer(
  connection: WalletConnection,
  account: string,
  toAccount: string,
  cCDAmount: string
) {
  const payload = {
    amount: CcdAmount.fromMicroCcd(cCDAmount ? cCDAmount : 0),
    toAddress: AccountAddress.fromBase58(toAccount),
  };

  console.debug("Sending simple ccd transfer transaction:");
  console.debug("Payload:");
  console.debug(payload);
  console.debug("Account:");
  console.debug(account);
  console.debug("");

  return connection.signAndSendTransaction(
    account,
    AccountTransactionType.Transfer,
    payload
  );
}

export async function simpleCCDTransferToNonExistingAccountAddress(
  connection: WalletConnection,
  account: string
) {
  const payload = {
    amount: CcdAmount.fromMicroCcd(1),
    toAddress: AccountAddress.fromBase58(
      "35CJPZohio6Ztii2zy1AYzJKvuxbGG44wrBn7hLHiYLoF2nxnh"
    ),
  };

  console.debug("Sending simple ccd transfer transaction:");
  console.debug("Payload:");
  console.debug(payload);
  console.debug("Account:");
  console.debug(account);
  console.debug("");

  return connection.signAndSendTransaction(
    account,
    AccountTransactionType.Transfer,
    payload
  );
}

export async function reverts(connection: WalletConnection, account: string) {
  const payload = {
    amount: CcdAmount.fromMicroCcd(0),
    address: ContractAddress.create(CONTRACT_INDEX, CONTRACT_SUB_INDEX),
    receiveName: ReceiveName.fromString(`${CONTRACT_NAME}.reverts`),
    maxContractExecutionEnergy: Energy.create(30000n),
  };

  console.debug("Sending update transaction:");
  console.debug("Payload:");
  console.debug(payload);
  console.debug("Account:");
  console.debug(account);
  console.debug("");

  return connection.signAndSendTransaction(
    account,
    AccountTransactionType.Update,
    payload
  );
}

export async function internalCallReverts(
  connection: WalletConnection,
  account: string
) {
  const payload = {
    amount: CcdAmount.fromMicroCcd(0),
    address: ContractAddress.create(CONTRACT_INDEX, CONTRACT_SUB_INDEX),
    receiveName: ReceiveName.fromString(
      `${CONTRACT_NAME}.internal_call_reverts`
    ),
    maxContractExecutionEnergy: Energy.create(30000n),
  };

  console.debug("Sending update transaction:");
  console.debug("Payload:");
  console.debug(payload);
  console.debug("Account:");
  console.debug(account);
  console.debug("");

  return connection.signAndSendTransaction(
    account,
    AccountTransactionType.Update,
    payload
  );
}

export async function internalCallSuccess(
  connection: WalletConnection,
  account: string
) {
  const payload = {
    amount: CcdAmount.fromMicroCcd(0),
    address: ContractAddress.create(CONTRACT_INDEX, CONTRACT_SUB_INDEX),
    receiveName: ReceiveName.fromString(
      `${CONTRACT_NAME}.internal_call_success`
    ),
    maxContractExecutionEnergy: Energy.create(30000n),
  };

  console.debug("Sending update transaction:");
  console.debug("Payload:");
  console.debug(payload);
  console.debug("Account:");
  console.debug(account);
  console.debug("");

  return connection.signAndSendTransaction(
    account,
    AccountTransactionType.Update,
    payload
  );
}

export async function notExistingEntrypoint(
  connection: WalletConnection,
  account: string
) {
  const schema = {
    parameters: 3,
    schema: typeSchemaFromBase64(SET_U8_PARAMETER_SCHEMA),
  };

  const payload = {
    amount: CcdAmount.fromMicroCcd(0),
    address: ContractAddress.create(CONTRACT_INDEX, CONTRACT_SUB_INDEX),
    receiveName: ReceiveName.fromString(`${CONTRACT_NAME}.does_not_exist`),
    maxContractExecutionEnergy: Energy.create(30000n),
  };

  console.debug("Sending update transaction:");
  console.debug("Schema:");
  console.debug(schema);
  console.debug("Payload:");
  console.debug(payload);
  console.debug("Account:");
  console.debug(account);
  console.debug("");

  return connection.signAndSendTransaction(
    account,
    AccountTransactionType.Update,
    payload,
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

export const state = createContext<State>({
  isConnected: false,
  account: undefined,
});
