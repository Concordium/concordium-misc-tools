/* eslint-disable no-console */
import React, { useEffect, useState, ChangeEvent } from 'react';
import Switch from 'react-switch';
import {
    toBuffer,
    JsonRpcClient,
    serializeTypeValue,
    deserializeTypeValue,
    deserializeReceiveReturnValue
} from '@concordium/web-sdk';
import { withJsonRpcClient, WalletConnectionProps, useConnection, useConnect } from '@concordium/react-components';
import { version } from '../package.json';

import { set_value, set_object, set_array, reverts, internal_call_reverts, internal_call_success, simple_CCD_transfer, simple_CCD_transfer_to_non_existing_account_address } from './utils';
import {
    CONTRACT_NAME,
    CONTRACT_INDEX,
    CONTRACT_SUB_INDEX,
    BROWSER_WALLET,
    VIEW_RETURN_VALUE_SCHEMA,
    WALLET_CONNECT,
    SET_OBJECT_PARAMETER_SCHEMA,
    REFRESH_INTERVAL,
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
    BASE_64_SCHEMA
} from './constants';

import { WalletConnectionTypeButton } from './WalletConnectorTypeButton';

const ButtonStyle = {
    color: 'white',
    borderRadius: 10,
    margin: '7px 0px 7px 0px',
    padding: '10px',
    width: '100%',
    border: '1px solid #26685D',
    backgroundColor: '#308274',
    cursor: 'pointer',
    fontWeight: 300,
    fontSize: '14px',
};

const ButtonStyleDisabled = {
    color: 'white',
    borderRadius: 10,
    margin: '7px 0px 7px 0px',
    padding: '10px',
    width: '100%',
    border: '1px solid #4B4A4A',
    backgroundColor: '#979797',
    cursor: 'pointer',
    fontWeight: 300,
    fontSize: '14px',
};

const InputFieldStyle = {
    backgroundColor: '#181817',
    color: 'white',
    borderRadius: 10,
    width: '100%',
    border: '1px solid #308274',
    margin: '7px 0px 7px 0px',
    padding: '10px 20px',
};

async function get_value(rpcClient: JsonRpcClient, useModuleSchema: boolean, dropDown: string) {

    let entrypointName = `${CONTRACT_NAME}.get_u8`;

    switch (dropDown) {
        case 'u8': entrypointName = `${CONTRACT_NAME}.get_u8`;
            break
        case 'u16': entrypointName = `${CONTRACT_NAME}.get_u16`;
            break
        case 'address': entrypointName = `${CONTRACT_NAME}.get_address`;
            break
        case 'contract_address': entrypointName = `${CONTRACT_NAME}.get_contract_address`;
            break
        case 'account_address': entrypointName = `${CONTRACT_NAME}.get_account_address`;
            break;
        case 'hash': entrypointName = `${CONTRACT_NAME}.get_hash`;
            break;
        case 'public_key': entrypointName = `${CONTRACT_NAME}.get_public_key`;
            break;
        case 'signature': entrypointName = `${CONTRACT_NAME}.get_signature`;
            break;
        case 'timestamp': entrypointName = `${CONTRACT_NAME}.get_timestamp`;
            break;
        case 'string': entrypointName = `${CONTRACT_NAME}.get_string`;
            break;
        case 'option_u8': entrypointName = `${CONTRACT_NAME}.get_option_u8`;
            break;
        // We call the `get_u8` function but later use the `timestamp` schema trying to deserialize the return value.
        case 'wrong_schema': entrypointName = `${CONTRACT_NAME}.get_u8`;
            break;
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
        case 'u8': schema = useModuleSchema ? BASE_64_SCHEMA : GET_U8_RETURN_VALUE_SCHEMA;
            break
        case 'u16': schema = useModuleSchema ? BASE_64_SCHEMA : GET_U16_RETURN_VALUE_SCHEMA;
            break
        case 'address': schema = useModuleSchema ? BASE_64_SCHEMA : GET_ADDRESS_RETURN_VALUE_SCHEMA;
            break
        case 'contract_address': schema = useModuleSchema ? BASE_64_SCHEMA : GET_CONTRACT_ADDRESS_RETURN_VALUE_SCHEMA;
            break
        case 'account_address': schema = useModuleSchema ? BASE_64_SCHEMA : GET_ACCOUNT_ADDRESS_RETURN_VALUE_SCHEMA;
            break;
        case 'hash': schema = useModuleSchema ? BASE_64_SCHEMA : GET_HASH_RETURN_VALUE_SCHEMA;
            break;
        case 'public_key': schema = useModuleSchema ? BASE_64_SCHEMA : GET_PUBLIC_KEY_RETURN_VALUE_SCHEMA;
            break;
        case 'signature': schema = useModuleSchema ? BASE_64_SCHEMA : GET_SIGNATURE_RETURN_VALUE_SCHEMA;
            break;
        case 'timestamp': schema = useModuleSchema ? BASE_64_SCHEMA : GET_TIMESTAMP_RETURN_VALUE_SCHEMA;
            break;
        case 'string': schema = useModuleSchema ? BASE_64_SCHEMA : GET_STRING_RETURN_VALUE_SCHEMA;
            break;
        case 'option_u8': schema = useModuleSchema ? BASE_64_SCHEMA : GET_OPTION_RETURN_VALUE_SCHEMA;
            break;
        // We called the `get_u8` function but now use the `timestamp` schema trying to deserialize the return value.
        case 'wrong_schema': schema = useModuleSchema ? BASE_64_SCHEMA : GET_TIMESTAMP_RETURN_VALUE_SCHEMA;
            dropDown = 'timestamp'
            break;
    }

    let returnValue = undefined;

    if (useModuleSchema) {
        try {
            returnValue = deserializeReceiveReturnValue(toBuffer(res.returnValue, 'hex'), toBuffer(schema, 'base64'), `${CONTRACT_NAME}`, `get_${dropDown}`)
        }
        catch (err) {
            throw new Error(
                (err as Error).message
            );
        }
    } else {
        try {
            returnValue = deserializeTypeValue
                (toBuffer(res.returnValue, 'hex'),
                    toBuffer(schema, 'base64')
                )
        }
        catch (err) {
            throw new Error(
                err as string
            );
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

async function view(rpcClient: JsonRpcClient) {

    const res = await rpcClient.invokeContract({
        method: `${CONTRACT_NAME}.view`,
        contract: { index: CONTRACT_INDEX, subindex: CONTRACT_SUB_INDEX },
    });

    if (!res || res.tag === 'failure' || !res.returnValue) {
        throw new Error(
            `RPC call 'invokeContract' on method '${CONTRACT_NAME}.view' of contract '${CONTRACT_INDEX}' failed`
        );
    }

    // @ts-ignore
    const state = deserializeTypeValue
        (toBuffer(res.returnValue, 'hex'),
            toBuffer(VIEW_RETURN_VALUE_SCHEMA, 'base64')
        );

    if (state === undefined) {
        throw new Error(
            `Deserializing the returnValue from the '${CONTRACT_NAME}.view' method of contract '${CONTRACT_INDEX}' failed`
        );
    } else {
        return JSON.stringify(state);
    }
}

async function account_info(rpcClient: JsonRpcClient, account: string) {
    return await rpcClient.getAccountInfo(account)
}

async function smart_contract_info(rpcClient: JsonRpcClient) {
    return await rpcClient.getInstanceInfo({ index: CONTRACT_INDEX, subindex: CONTRACT_SUB_INDEX })
}

export default function Transactions(props: WalletConnectionProps) {

    const { network, activeConnectorType, activeConnector, activeConnectorError, connectedAccounts, genesisHashes } =
        props;


    const { connection, setConnection, account, genesisHash } = useConnection(connectedAccounts, genesisHashes);
    const { connect, isConnecting, connectError } = useConnect(activeConnector, setConnection);

    const [publicKeyError, setPublicKeyError] = useState('');

    const [publicKey, setPublicKey] = useState('');
    const [nextNonce, setNextNonce] = useState<number>(0);
    const [nonce, setNonce] = useState('');

    const [record, setRecord] = useState('');

    const [returnValue, setReturnValue] = useState('');

    const [accountBalance, setAccountBalance] = useState('');
    const [smartContractBalance, setSmartContractBalance] = useState('');

    const [cCDAmount, setCCDAmount] = useState('');
    const [input, setInput] = useState('');

    const [useModuleSchema, setUseModuleSchema] = useState(true);
    const [isPayable, setIsPayable] = useState(true);
    const [dropDown, setDropDown] = useState('u8');
    const [dropDown2, setDropDown2] = useState('u8');
    const [returnValueError, setReturnValueError] = useState('');

    const [toAccount, setToAccount] = useState('');

    const [signature, setSignature] = useState('');
    const [signingError, setSigningError] = useState('');

    const changeInputHandler = (event: ChangeEvent) => {
        const target = event.target as HTMLTextAreaElement;
        setInput(target.value);
    };

    const changeCCDAmountHandler = (event: ChangeEvent) => {
        const target = event.target as HTMLTextAreaElement;
        setCCDAmount(target.value);
    };

    const changeMessageHandler = (event: ChangeEvent) => {
        const target = event.target as HTMLTextAreaElement;
        setMessage(target.value);
    };

    const changeDropDownHandler = (event: ChangeEvent) => {
        var e = (document.getElementById("function")) as HTMLSelectElement;
        var sel = e.selectedIndex;
        var value = e.options[sel].value;
        setDropDown(value);
    };

    const changeDropDown2Handler = (event: ChangeEvent) => {
        var e = (document.getElementById("function2")) as HTMLSelectElement;
        var sel = e.selectedIndex;
        var value = e.options[sel].value;
        setDropDown2(value);
    };

    const changeToAccountHandler = (event: ChangeEvent) => {
        const target = event.target as HTMLTextAreaElement;
        setToAccount(target.value);
    };


    // // Refresh account_info periodically.
    // // eslint-disable-next-line consistent-return
    useEffect(() => {
        if (connection && account) {
            const interval = setInterval(() => {
                console.log('refreshing1');
                withJsonRpcClient(connection, (rpcClient) => account_info(rpcClient, account))
                    .then((returnValue) => {
                        if (returnValue !== undefined) {
                            setAccountBalance(returnValue.accountAmount.toString());
                        }
                        setPublicKeyError('');
                    })
                    .catch((e) => {
                        setAccountBalance('');
                        setPublicKeyError((e as Error).message);
                        setPublicKey('');
                        setNextNonce(0);
                        setNonce('');
                    });
            }, REFRESH_INTERVAL.asMilliseconds());
            return () => clearInterval(interval);
        }
    }, [connection, account]);

    // // Refresh smart_contract_info periodically.
    // // eslint-disable-next-line consistent-return
    useEffect(() => {
        if (connection) {
            const interval = setInterval(() => {
                console.log('refreshing2');
                withJsonRpcClient(connection, (rpcClient) => smart_contract_info(rpcClient))
                    .then((returnValue) => {
                        if (returnValue !== undefined) {
                            setSmartContractBalance(returnValue.amount.microCcdAmount.toString());
                        }
                        setPublicKeyError('');
                    })
                    .catch((e) => {
                        setSmartContractBalance('');
                        setPublicKeyError((e as Error).message);
                        setPublicKey('');
                        setNextNonce(0);
                        setNonce('');
                    });
            }, REFRESH_INTERVAL.asMilliseconds());
            return () => clearInterval(interval);
        }
    }, [connection, account]);

    // // Refresh view periodically.
    // // eslint-disable-next-line consistent-return
    useEffect(() => {
        if (connection && account) {
            const interval = setInterval(() => {
                console.log('refreshing3');
                withJsonRpcClient(connection, (rpcClient) => view(rpcClient))
                    .then((returnValue) => {
                        if (returnValue !== undefined) {
                            setRecord(returnValue);
                        }
                        setPublicKeyError('');
                    })
                    .catch((e) => {
                        setRecord('');
                        setPublicKeyError((e as Error).message);
                        setPublicKey('');
                        setNextNonce(0);
                        setNonce('');
                    });
            }, REFRESH_INTERVAL.asMilliseconds());
            return () => clearInterval(interval);
        }
    }, [connection, account]);

    useEffect(() => {
        if (connection && account) {
            withJsonRpcClient(connection, (rpcClient) => account_info(rpcClient, account))
                .then((returnValue) => {
                    if (returnValue !== undefined) {
                        setAccountBalance(returnValue.accountAmount.toString());
                    }
                    setPublicKeyError('');
                })
                .catch((e) => {
                    setPublicKeyError((e as Error).message);
                    setPublicKey('');
                    setNextNonce(0);
                    setNonce('');
                });
        }
    }, [connection]);

    useEffect(() => {
        if (connection && account) {
            withJsonRpcClient(connection, (rpcClient) => smart_contract_info(rpcClient))
                .then((returnValue) => {
                    if (returnValue !== undefined) {
                        setSmartContractBalance(returnValue.amount.microCcdAmount.toString());
                    }
                    setPublicKeyError('');
                })
                .catch((e) => {
                    setPublicKeyError((e as Error).message);
                    setPublicKey('');
                    setNextNonce(0);
                    setNonce('');
                });
        }
    }, [connection]);

    useEffect(() => {
        if (connection && account) {
            withJsonRpcClient(connection, (rpcClient) => view(rpcClient))
                .then((returnValue) => {
                    if (returnValue !== undefined) {
                        setRecord(returnValue);
                    }
                    setPublicKeyError('');
                })
                .catch((e) => {
                    setPublicKeyError((e as Error).message);
                    setPublicKey('');
                    setNextNonce(0);
                    setNonce('');
                });
        }
    }, [connection]);

    const [isRegisterPublicKeyPage, setIsRegisterPublicKeyPage] = useState(true);
    const [txHash, setTxHash] = useState('');
    const [message, setMessage] = useState('');
    const [transactionError, setTransactionError] = useState('');

    const [isWaitingForTransaction, setWaitingForUser] = useState(false);
    return (
        <div>
            <div className="centerLargeText">Version: {version}</div>
            <h1 className="header">Wallet Connect / Browser Wallet Testing Bench </h1>
            <div className="containerSpaceBetween">
                <WalletConnectionTypeButton
                    buttonStyle={ButtonStyle}
                    disabledButtonStyle={ButtonStyleDisabled}
                    connectorType={BROWSER_WALLET}
                    connectorName="Browser Wallet"
                    setWaitingForUser={setWaitingForUser}
                    connection={connection}
                    {...props}
                />
                <WalletConnectionTypeButton
                    buttonStyle={ButtonStyle}
                    disabledButtonStyle={ButtonStyleDisabled}
                    connectorType={WALLET_CONNECT}
                    connectorName="Wallet Connect"
                    setWaitingForUser={setWaitingForUser}
                    connection={connection}
                    {...props}
                />
            </div>
            <div>
                {activeConnectorError && <p style={{ color: 'red' }}>Connector Error: {activeConnectorError}.</p>}
                {!activeConnectorError && !isWaitingForTransaction && activeConnectorType && !activeConnector && (
                    <p>
                        <i>Loading connector...</i>
                    </p>
                )}
                {connectError && <p style={{ color: 'red' }}>Connect Error: {connectError}.</p>}
                {!connection && !isWaitingForTransaction && activeConnectorType && activeConnector && (
                    <p>
                        <button style={ButtonStyle} type="button" onClick={connect}>
                            {isConnecting && 'Connecting...'}
                            {!isConnecting && activeConnectorType === BROWSER_WALLET && 'Connect Browser Wallet'}
                            {!isConnecting && activeConnectorType === WALLET_CONNECT && 'Connect Mobile Wallet'}
                        </button>
                    </p>
                )}
                {account && (
                    <>
                        <div className="centerLargeText">Connected account:</div>
                        <br></br>
                        <div className="containerSwitch">
                            <button
                                className="link"
                                type="button"
                                onClick={() => {
                                    window.open(
                                        `https://testnet.ccdscan.io/?dcount=1&dentity=account&daddress=${account}`,
                                        '_blank',
                                        'noopener,noreferrer'
                                    );
                                }}
                            >
                                {account}
                            </button>
                        </div>
                        <br></br>
                        <div className="centerLargeText">Your account balance:</div>
                        <br></br>
                        <div className="centerLargeText">
                            {accountBalance} microCCD
                        </div>
                        <br></br>
                        <div className="centerLargeText">Smart contract balance:</div>
                        <br></br>
                        <div className="centerLargeText">
                            {smartContractBalance} microCCD
                        </div>
                        <br />
                        <br />
                        {true && (
                            <>
                                <div className="centerLargeText">Error or Transaction status{txHash === '' ? ':' : ' (May take a moment to finalize):'}</div>    <br />
                                {!txHash && !transactionError && <div className="centerLargeText">None</div>}
                                {!txHash && transactionError && (
                                    <div style={{ color: 'red' }}>Error: {transactionError}.</div>
                                )}
                                <div className="containerSwitch">
                                    {txHash && (
                                        <>
                                            <button
                                                className="link"
                                                type="button"
                                                onClick={() => {
                                                    window.open(
                                                        `https://testnet.ccdscan.io/?dcount=1&dentity=transaction&dhash=${txHash}`,
                                                        '_blank',
                                                        'noopener,noreferrer'
                                                    );
                                                }}
                                            >
                                                {txHash}
                                            </button>
                                            <br />
                                        </>
                                    )}
                                </div>
                            </>
                        )}
                        <br></br>
                        <div className="centerLargeText"> The smart contract state: </div>
                        <pre className="centerLargeText">{record}</pre>
                        <br />
                        <div className="dashedLine"></div>
                        <div className="centerLargeText">Testing simple input parameters:</div>
                        <br />
                        {connection && account !== undefined && !publicKey && (

                            <>
                                <div className="containerSpaceBetween">
                                    <div className="centerLargeText">Use module schema</div>
                                    <Switch
                                        onChange={() => {
                                            setUseModuleSchema(!useModuleSchema);
                                        }}
                                        onColor="#308274"
                                        offColor="#308274"
                                        onHandleColor="#174039"
                                        offHandleColor="#174039"
                                        checked={!useModuleSchema}
                                        checkedIcon={false}
                                        uncheckedIcon={false}
                                    />
                                    <div className="centerLargeText">Use parameter schema</div>
                                </div>
                                <div className="containerSpaceBetween">
                                    <div className="centerLargeText">Is payable</div>
                                    <Switch
                                        onChange={() => {
                                            setIsPayable(!isPayable);
                                        }}
                                        onColor="#308274"
                                        offColor="#308274"
                                        onHandleColor="#174039"
                                        offHandleColor="#174039"
                                        checked={!isPayable}
                                        checkedIcon={false}
                                        uncheckedIcon={false}
                                    />
                                    <div className="centerLargeText">Is not payable</div>
                                </div>
                                <br></br>
                                <div className="centerLargeText">Select function:</div>
                                <br></br>
                                <div className="containerSpaceBetween">
                                    <div></div>
                                    <select className="centerLargeBlackText" name="function2" id="function2" onChange={changeDropDown2Handler}>
                                        <option value="u8" selected>u8</option>
                                        <option value="u16">u16</option>
                                        <option value="address">Address</option>
                                        <option value="contract_address">ContractAddress</option>
                                        <option value="account_address">AccountAddress</option>
                                        <option value="hash">Hash</option>
                                        <option value="public_key">PublicKey</option>
                                        <option value="signature">Signature</option>
                                        <option value="timestamp">Timestamp</option>
                                        <option value="string">String</option>
                                        <option value="option_u8_none">Option (None)</option>
                                        <option value="option_u8_some">Option (Some)</option>
                                    </select>
                                    <div></div>
                                </div>
                                <label>
                                    <p className="centerLargeText">micro CCD:</p>
                                    <input
                                        className="input"
                                        style={InputFieldStyle}
                                        id="CCDAmount"
                                        type="text"
                                        placeholder="0"
                                        onChange={changeCCDAmountHandler}
                                    />
                                </label>
                                <label>
                                    <p className="centerLargeText">Input parameter:</p>
                                    <input
                                        className="input"
                                        style={InputFieldStyle}
                                        id="input"
                                        type="text"
                                        placeholder='5 | 15 | {"Contract":[{"index":3,"subindex":0}]} or {"Account":["4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt"]} | {"index":3,"subindex":0} | 4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt | 18ee24150dcb1d96752a4d6dd0f20dfd8ba8c38527e40aa8509b7adecf78f9c6 | 37a2a8e52efad975dbf6580e7734e4f249eaa5ea8a763e934a8671cd7e446499 | 632f567c9321405ce201a0a38615da41efe259ede154ff45ad96cdf860718e79bde07cff72c4d119c644552a8c7f0c413f5cf5390b0ea0458993d6d6374bd904 | 2030-08-08T05:15:00Z | aaa | | 3 |'
                                        onChange={changeInputHandler}
                                    />
                                </label>
                                <button
                                    style={ButtonStyle}
                                    type="button"
                                    onClick={() => {
                                        setTxHash('');
                                        setTransactionError('');
                                        setWaitingForUser(true);
                                        const tx = set_value(connection, account, useModuleSchema, isPayable, dropDown2, input, cCDAmount);
                                        tx.then(setTxHash)
                                            .catch((err: Error) => setTransactionError((err as Error).message))
                                            .finally(() => setWaitingForUser(false));
                                    }}
                                >
                                    Set {dropDown2} value
                                </button>
                                <div className="dashedLine"></div>
                                <div className="centerLargeText">Testing return value deserialization of functions:</div>
                                <div className="containerSpaceBetween">
                                    <div className="centerLargeText">Use module schema</div>
                                    <Switch
                                        onChange={() => {
                                            setUseModuleSchema(!useModuleSchema);
                                        }}
                                        onColor="#308274"
                                        offColor="#308274"
                                        onHandleColor="#174039"
                                        offHandleColor="#174039"
                                        checked={!useModuleSchema}
                                        checkedIcon={false}
                                        uncheckedIcon={false}
                                    />
                                    <div className="centerLargeText">Use parameter schema</div>
                                </div>
                                <br></br>
                                <div className="centerLargeText">Select function:</div>
                                <br></br>
                                <div className="containerSpaceBetween">
                                    <div></div>
                                    <select className="centerLargeBlackText" name="function" id="function" onChange={changeDropDownHandler}>
                                        <option value="u8" selected>u8</option>
                                        <option value="u16">u16</option>
                                        <option value="address">Address</option>
                                        <option value="contract_address">ContractAddress</option>
                                        <option value="account_address">AccountAddress</option>
                                        <option value="hash">Hash</option>
                                        <option value="public_key">PublicKey</option>
                                        <option value="signature">Signature</option>
                                        <option value="timestamp">Timestamp</option>
                                        <option value="string">String</option>
                                        <option value="option_u8">Option</option>
                                        <option value="wrong_schema">Wrong schema (error should be returned)</option>
                                    </select>
                                    <div></div>
                                </div>
                                <button
                                    style={ButtonStyle}
                                    type="button"
                                    onClick={() => {
                                        setReturnValue('');
                                        setReturnValueError('');
                                        withJsonRpcClient(connection, (rpcClient) => get_value(rpcClient, useModuleSchema, dropDown))
                                            .then((value) => {
                                                if (value !== undefined) {
                                                    setReturnValue(JSON.stringify(value));
                                                }
                                            })
                                            .catch((e) => {
                                                setReturnValueError((e as Error).message);
                                            });
                                    }}
                                >
                                    Get {dropDown} value
                                </button>
                                {returnValue !== '' && (<>
                                    <div className="centerLargeText">Your return value is:</div>
                                    <div className="centerLargeText">{returnValue}</div>
                                </>)}
                                {!returnValue && returnValueError && (
                                    <div style={{ color: 'red' }}>Error: {returnValueError}.</div>
                                )}
                                <br />
                                <div className="dashedLine"></div>
                                <div className="centerLargeText">Testing complex object as input parameter:</div>
                                <br />
                                <br />
                                <div className="containerSpaceBetween">
                                    <div className="centerLargeText">Use module schema</div>
                                    <Switch
                                        onChange={() => {
                                            setUseModuleSchema(!useModuleSchema);
                                        }}
                                        onColor="#308274"
                                        offColor="#308274"
                                        onHandleColor="#174039"
                                        offHandleColor="#174039"
                                        checked={!useModuleSchema}
                                        checkedIcon={false}
                                        uncheckedIcon={false}
                                    />
                                    <div className="centerLargeText">Use parameter schema</div>
                                </div>
                                <div className="containerSpaceBetween">
                                    <div className="centerLargeText">Is payable</div>
                                    <Switch
                                        onChange={() => {
                                            setIsPayable(!isPayable);
                                        }}
                                        onColor="#308274"
                                        offColor="#308274"
                                        onHandleColor="#174039"
                                        offHandleColor="#174039"
                                        checked={!isPayable}
                                        checkedIcon={false}
                                        uncheckedIcon={false}
                                    />
                                    <div className="centerLargeText">Is not payable</div>
                                </div>
                                <label>
                                    <p className="centerLargeText">micro CCD:</p>
                                    <input
                                        className="input"
                                        style={InputFieldStyle}
                                        id="CCDAmount"
                                        type="text"
                                        placeholder="0"
                                        onChange={changeCCDAmountHandler}
                                    />
                                </label>
                                <button
                                    style={ButtonStyle}
                                    type="button"
                                    onClick={() => {
                                        setTxHash('');
                                        setTransactionError('');
                                        setWaitingForUser(true);
                                        const tx = set_object(connection, account, useModuleSchema, isPayable, cCDAmount);
                                        tx.then(setTxHash)
                                            .catch((err: Error) => setTransactionError((err as Error).message))
                                            .finally(() => setWaitingForUser(false));
                                    }}
                                >
                                    Set object
                                </button>
                                <br />
                                <div className="dashedLine"></div>
                                <div className="centerLargeText">Testing array as input parameter:</div>
                                <div className="containerSpaceBetween">
                                    <div className="centerLargeText">Use module schema</div>
                                    <Switch
                                        onChange={() => {
                                            setUseModuleSchema(!useModuleSchema);
                                        }}
                                        onColor="#308274"
                                        offColor="#308274"
                                        onHandleColor="#174039"
                                        offHandleColor="#174039"
                                        checked={!useModuleSchema}
                                        checkedIcon={false}
                                        uncheckedIcon={false}
                                    />
                                    <div className="centerLargeText">Use parameter schema</div>
                                </div>
                                <div className="containerSpaceBetween">
                                    <div className="centerLargeText">Is payable</div>
                                    <Switch
                                        onChange={() => {
                                            setIsPayable(!isPayable);
                                        }}
                                        onColor="#308274"
                                        offColor="#308274"
                                        onHandleColor="#174039"
                                        offHandleColor="#174039"
                                        checked={!isPayable}
                                        checkedIcon={false}
                                        uncheckedIcon={false}
                                    />
                                    <div className="centerLargeText">Is not payable</div>
                                </div>
                                <br />
                                <br />
                                <label>
                                    <p className="centerLargeText">micro CCD:</p>
                                    <input
                                        className="input"
                                        style={InputFieldStyle}
                                        id="CCDAmount"
                                        type="text"
                                        placeholder="0"
                                        onChange={changeCCDAmountHandler}
                                    />
                                </label>
                                <button
                                    style={ButtonStyle}
                                    type="button"
                                    onClick={() => {
                                        setTxHash('');
                                        setTransactionError('');
                                        setWaitingForUser(true);
                                        const tx = set_array(connection, account, useModuleSchema, isPayable, cCDAmount);
                                        tx.then(setTxHash)
                                            .catch((err: Error) => setTransactionError((err as Error).message))
                                            .finally(() => setWaitingForUser(false));
                                    }}
                                >
                                    Set Array
                                </button>
                                <br />
                                <div className="dashedLine"></div>
                                <div className="centerLargeText">Testing calling a function that calls another smart contract successfully:</div>
                                <button
                                    style={ButtonStyle}
                                    type="button"
                                    onClick={() => {
                                        setTxHash('');
                                        setTransactionError('');
                                        setWaitingForUser(true);
                                        const tx = internal_call_success(connection, account);
                                        tx.then(setTxHash)
                                            .catch((err: Error) => setTransactionError((err as Error).message))
                                            .finally(() => setWaitingForUser(false));
                                    }}
                                >
                                    Success (internal call to smart contract)
                                </button>
                                <br />
                                <div className="dashedLine"></div>
                                <div className="centerLargeText">Testing calling a function that reverts due to the smart contract logic:</div>
                                <button
                                    style={ButtonStyle}
                                    type="button"
                                    onClick={() => {
                                        setTxHash('');
                                        setTransactionError('');
                                        setWaitingForUser(true);
                                        const tx = reverts(connection, account);
                                        tx.then(setTxHash)
                                            .catch((err: Error) => setTransactionError((err as Error).message))
                                            .finally(() => setWaitingForUser(false));
                                    }}
                                >
                                    Revert
                                </button>
                                <br />
                                <div className="dashedLine"></div>
                                <div className="centerLargeText">Testing calling a function that reverts due to an internal call that reverts:</div>
                                <button
                                    style={ButtonStyle}
                                    type="button"
                                    onClick={() => {
                                        setTxHash('');
                                        setTransactionError('');
                                        setWaitingForUser(true);
                                        const tx = internal_call_reverts(connection, account);
                                        tx.then(setTxHash)
                                            .catch((err: Error) => setTransactionError((err as Error).message))
                                            .finally(() => setWaitingForUser(false));
                                    }}
                                >
                                    Revert (internal call reverts)
                                </button>
                                <br />
                                <div className="dashedLine"></div>
                                <div className="centerLargeText">Testing simple CCD transfer:</div>
                                <label>
                                    <p className="centerLargeText">micro CCD:</p>
                                    <input
                                        className="input"
                                        style={InputFieldStyle}
                                        id="CCDAmount"
                                        type="text"
                                        placeholder="0"
                                        onChange={changeCCDAmountHandler}
                                    />
                                </label>
                                <label>
                                    <p className="centerLargeText">To account:</p>
                                    <input
                                        className="input"
                                        style={InputFieldStyle}
                                        id="toAccount"
                                        type="text"
                                        placeholder="4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt"
                                        onChange={changeToAccountHandler}
                                    />
                                </label>
                                <button
                                    style={ButtonStyle}
                                    type="button"
                                    onClick={() => {
                                        setTxHash('');
                                        setTransactionError('');
                                        setWaitingForUser(true);
                                        const tx = simple_CCD_transfer(connection, account, toAccount, cCDAmount);
                                        tx.then(setTxHash)
                                            .catch((err: Error) => setTransactionError((err as Error).message))
                                            .finally(() => setWaitingForUser(false));
                                    }}
                                >
                                    Send simple CCD transfer
                                </button>
                                <br />
                                <div className="dashedLine"></div>
                                <div className="centerLargeText">Testing simple CCD transfer to non exising account address:</div>
                                <button
                                    style={ButtonStyle}
                                    type="button"
                                    onClick={() => {
                                        setTxHash('');
                                        setTransactionError('');
                                        setWaitingForUser(true);
                                        const tx = simple_CCD_transfer_to_non_existing_account_address(connection, account);
                                        tx.then(setTxHash)
                                            .catch((err: Error) => setTransactionError((err as Error).message))
                                            .finally(() => setWaitingForUser(false));
                                    }}
                                >
                                    Send simple CCD transfer to non existing account address (reverts)
                                </button>
                                <br />
                                <div className="dashedLine"></div>
                                <div className="centerLargeText">Testing signing a string message with the wallet:</div>
                                <label>
                                    <p className="centerLargeText">Message to be signed:</p>
                                    <input
                                        className="input"
                                        style={InputFieldStyle}
                                        id="message"
                                        type="text"
                                        placeholder="My message"
                                        onChange={changeMessageHandler}
                                    />
                                </label>
                                <button
                                    style={ButtonStyle}
                                    type="button"
                                    onClick={() => {
                                        setSignature('');
                                        const promise = connection.signMessage(account,
                                            {
                                                type: 'StringMessage',
                                                value: message,
                                            })
                                        promise
                                            .then((permitSignature) => {
                                                setSignature(permitSignature[0][0]);
                                            })
                                            .catch((err: Error) => setSigningError((err as Error).message));
                                    }}
                                >
                                    Sign message
                                </button>
                                {signingError && <div style={{ color: 'red' }}>Error: {signingError}.</div>}
                                {signature !== '' && (
                                    <>
                                        <div className="centerLargeText"> Your generated signature is: </div>
                                        <div className="centerLargeText">{signature}</div>
                                    </>
                                )}
                                <br />
                                <div className="dashedLine"></div>
                                <div className="centerLargeText">Testing signing a byte message with the wallet:</div>
                                <label>
                                    <p className="centerLargeText">Message to be signed:</p>
                                    <input
                                        className="input"
                                        style={InputFieldStyle}
                                        id="message"
                                        type="text"
                                        placeholder="My message"
                                        onChange={changeMessageHandler}
                                    />
                                </label>
                                <button
                                    style={ButtonStyle}
                                    type="button"
                                    onClick={() => {

                                        const signMessage = {
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

                                        const serializedMessage = serializeTypeValue(
                                            signMessage,
                                            toBuffer(SET_OBJECT_PARAMETER_SCHEMA, 'base64')
                                        );

                                        setSignature('');
                                        const promise = connection.signMessage(account,
                                            {
                                                type: 'BinaryMessage',
                                                value: serializedMessage,
                                                schema: {
                                                    type: 'TypeSchema',
                                                    value: toBuffer(SET_OBJECT_PARAMETER_SCHEMA, 'base64')
                                                },
                                            })
                                        promise
                                            .then((permitSignature) => {
                                                setSignature(permitSignature[0][0]);
                                            })
                                            .catch((err: Error) => setSigningError((err as Error).message));
                                    }}
                                >
                                    Sign message
                                </button>
                                {signingError && <div style={{ color: 'red' }}>Error: {signingError}.</div>}
                                {signature !== '' && (
                                    <>
                                        <div className="centerLargeText"> Your generated signature is: </div>
                                        <div className="centerLargeText">{signature}</div>
                                    </>
                                )}
                                <br />
                            </>
                        )}
                    </>
                )}
            </div>
        </div>
    );
}
