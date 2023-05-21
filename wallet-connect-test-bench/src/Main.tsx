/* eslint-disable no-console */
import React, { useEffect, useState, ChangeEvent } from 'react';
import Switch from 'react-switch';
import { toBuffer, serializeTypeValue } from '@concordium/web-sdk';
import { withJsonRpcClient, WalletConnectionProps, useConnection, useConnect } from '@concordium/react-components';
import { version } from '../package.json';
import { WalletConnectionTypeButton } from './WalletConnectorTypeButton';

import { smartContractInfo, accountInfo, view, getValue } from './reading_from_blockchain';
import {
    setValue,
    setObject,
    setArray,
    reverts,
    internalCallReverts,
    internalCallSuccess,
    notExistingEntrypoint,
    simpleCCDTransfer,
    simpleCCDTransferToNonExistingAccountAddress,
} from './writing_to_blockchain';

import { BROWSER_WALLET, WALLET_CONNECT, SET_OBJECT_PARAMETER_SCHEMA, REFRESH_INTERVAL } from './constants';

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

export default function Main(props: WalletConnectionProps) {
    const { activeConnectorType, activeConnector, activeConnectorError, connectedAccounts, genesisHashes } = props;

    const { connection, setConnection, account } = useConnection(connectedAccounts, genesisHashes);
    const { connect, isConnecting, connectError } = useConnect(activeConnector, setConnection);

    const [viewError, setViewError] = useState('');
    const [returnValueError, setReturnValueError] = useState('');
    const [signingError, setSigningError] = useState('');
    const [transactionError, setTransactionError] = useState('');

    const [record, setRecord] = useState('');
    const [returnValue, setReturnValue] = useState('');
    const [isWaitingForTransaction, setWaitingForUser] = useState(false);

    const [accountBalance, setAccountBalance] = useState('');
    const [smartContractBalance, setSmartContractBalance] = useState('');

    const [cCDAmount, setCCDAmount] = useState('');
    const [input, setInput] = useState('');

    const [useModuleSchema, setUseModuleSchema] = useState(true);
    const [isPayable, setIsPayable] = useState(true);
    const [readDropDown, setReadDropDown] = useState('u8');
    const [writeDropDown, setWriteDropDown] = useState('u8');
    const [toAccount, setToAccount] = useState('');
    const [signature, setSignature] = useState('');
    const [byteSignature, setByteSignature] = useState('');

    const [txHash, setTxHash] = useState('');
    const [message, setMessage] = useState('');

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

    const changeReadDropDownHandler = () => {
        const e = document.getElementById('read') as HTMLSelectElement;
        const sel = e.selectedIndex;
        const { value } = e.options[sel];
        setReadDropDown(value);
    };

    const changeWriteDropDownHandler = () => {
        const e = document.getElementById('write') as HTMLSelectElement;
        const sel = e.selectedIndex;
        const { value } = e.options[sel];
        setWriteDropDown(value);
    };

    const changeToAccountHandler = (event: ChangeEvent) => {
        const target = event.target as HTMLTextAreaElement;
        setToAccount(target.value);
    };

    // Refresh accountInfo periodically.
    // eslint-disable-next-line consistent-return
    useEffect(() => {
        if (connection && account) {
            const interval = setInterval(() => {
                console.log('refreshing1');
                withJsonRpcClient(connection, (rpcClient) => accountInfo(rpcClient, account))
                    .then((value) => {
                        if (value !== undefined) {
                            setAccountBalance(value.accountAmount.toString());
                        }
                        setViewError('');
                    })
                    .catch((e) => {
                        setAccountBalance('');
                        setViewError((e as Error).message);
                    });
            }, REFRESH_INTERVAL.asMilliseconds());
            return () => clearInterval(interval);
        }
    }, [connection, account]);

    // Refresh smartContractInfo periodically.
    // eslint-disable-next-line consistent-return
    useEffect(() => {
        if (connection) {
            const interval = setInterval(() => {
                console.log('refreshing2');
                withJsonRpcClient(connection, (rpcClient) => smartContractInfo(rpcClient))
                    .then((value) => {
                        if (value !== undefined) {
                            setSmartContractBalance(value.amount.microCcdAmount.toString());
                        }
                        setViewError('');
                    })
                    .catch((e) => {
                        setSmartContractBalance('');
                        setViewError((e as Error).message);
                    });
            }, REFRESH_INTERVAL.asMilliseconds());
            return () => clearInterval(interval);
        }
    }, [connection, account]);

    // Refresh view periodically.
    // eslint-disable-next-line consistent-return
    useEffect(() => {
        if (connection && account) {
            const interval = setInterval(() => {
                console.log('refreshing3');
                withJsonRpcClient(connection, (rpcClient) => view(rpcClient))
                    .then((value) => {
                        if (value !== undefined) {
                            setRecord(JSON.parse(value));
                        }
                        setViewError('');
                    })
                    .catch((e) => {
                        setRecord('');
                        setViewError((e as Error).message);
                    });
            }, REFRESH_INTERVAL.asMilliseconds());
            return () => clearInterval(interval);
        }
    }, [connection, account]);

    useEffect(() => {
        if (connection && account) {
            withJsonRpcClient(connection, (rpcClient) => accountInfo(rpcClient, account))
                .then((value) => {
                    if (value !== undefined) {
                        setAccountBalance(value.accountAmount.toString());
                    }
                    setViewError('');
                })
                .catch((e) => {
                    setViewError((e as Error).message);
                    setAccountBalance('');
                });
        }
    }, [connection]);

    useEffect(() => {
        if (connection && account) {
            withJsonRpcClient(connection, (rpcClient) => smartContractInfo(rpcClient))
                .then((value) => {
                    if (value !== undefined) {
                        setSmartContractBalance(value.amount.microCcdAmount.toString());
                    }
                    setViewError('');
                })
                .catch((e) => {
                    setViewError((e as Error).message);
                    setSmartContractBalance('');
                });
        }
    }, [connection]);

    useEffect(() => {
        if (connection && account) {
            withJsonRpcClient(connection, (rpcClient) => view(rpcClient))
                .then((value) => {
                    if (value !== undefined) {
                        setRecord(JSON.parse(value));
                    }
                    setViewError('');
                })
                .catch((e) => {
                    setViewError((e as Error).message);
                    setRecord('');
                });
        }
    }, [connection]);

    return (
        <div className="centerLargeText">
            <div>Version: {version}</div>
            <h1>Wallet Connect / Browser Wallet Testing Bench </h1>
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
                        <button className="buttonStyle" type="button" onClick={connect}>
                            {isConnecting && 'Connecting...'}
                            {!isConnecting && activeConnectorType === BROWSER_WALLET && 'Connect Browser Wallet'}
                            {!isConnecting && activeConnectorType === WALLET_CONNECT && 'Connect Mobile Wallet'}
                        </button>
                    </p>
                )}
                {account && (
                    <>
                        <div>Connected account:</div>
                        <br />
                        <div>
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
                        <br />
                        <div>Your account balance:</div>
                        <br />
                        <div>{accountBalance} CCD (micro)</div>
                        <br />
                        <div>Smart contract balance:</div>
                        <br />
                        <div>{smartContractBalance} CCD (micro)</div>
                        <br />
                        <div>Smart contract state:</div>
                        <pre className="largeText">{JSON.stringify(record, null, '\t')}</pre>
                        <br />
                        <br />
                        <div>
                            Error or Transaction status{txHash === '' ? ':' : ' (May take a moment to finalize):'}
                        </div>
                        <br />
                        {!txHash && !transactionError && (
                            <div style={{ color: 'red' }}>
                                IMPORTANT: After pressing a button that should send a transaction, the transaction hash
                                or error returned by the wallet are displayed HERE.
                            </div>
                        )}
                        {!txHash && transactionError && <div style={{ color: 'red' }}>Error: {transactionError}.</div>}
                        {viewError && <div style={{ color: 'red' }}>Error: {viewError}.</div>}
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
                        <br />
                        <div className="dashedLine" />
                        <div>Testing simple input parameters:</div>
                        <br />
                        {connection && account !== undefined && (
                            <>
                                <div className="containerSpaceBetween">
                                    <div>Use module schema</div>
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
                                    <div>Use parameter schema</div>
                                </div>
                                <div className="containerSpaceBetween">
                                    <div>Is payable</div>
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
                                    <div>Is not payable</div>
                                </div>
                                <br />
                                <div>Select function:</div>
                                <br />
                                <div className="containerSpaceBetween">
                                    <div />
                                    <select
                                        className="centerLargeText"
                                        name="write"
                                        id="write"
                                        onChange={changeWriteDropDownHandler}
                                    >
                                        <option value="u8" selected>
                                            u8
                                        </option>
                                        <option value="u16">u16</option>
                                        <option value="address">Address</option>
                                        <option value="contract_address">ContractAddress</option>
                                        <option value="account_address">AccountAddress</option>
                                        <option value="hash">Hash</option>
                                        <option value="public_key">Public key</option>
                                        <option value="signature">Signature</option>
                                        <option value="timestamp">Timestamp</option>
                                        <option value="string">String</option>
                                        <option value="option_u8_none">Option (None)</option>
                                        <option value="option_u8_some">Option (Some)</option>
                                        <option value="wrong_schema">Wrong schema (error should be returned)</option>
                                    </select>
                                    <div />
                                </div>
                                <label>
                                    <p>CCD (micro):</p>
                                    <input
                                        className="inputFieldStyle"
                                        id="CCDAmount"
                                        type="text"
                                        placeholder="0"
                                        onChange={changeCCDAmountHandler}
                                    />
                                </label>
                                <label>
                                    <p>Input parameter:</p>
                                    <input
                                        className="inputFieldStyle"
                                        id="input"
                                        type="text"
                                        placeholder='5 | 15 | {"Contract":[{"index":3,"subindex":0}]} or {"Account":["4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt"]} | {"index":3,"subindex":0} | 4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt | 18ee24150dcb1d96752a4d6dd0f20dfd8ba8c38527e40aa8509b7adecf78f9c6 | 37a2a8e52efad975dbf6580e7734e4f249eaa5ea8a763e934a8671cd7e446499 | 632f567c9321405ce201a0a38615da41efe259ede154ff45ad96cdf860718e79bde07cff72c4d119c644552a8c7f0c413f5cf5390b0ea0458993d6d6374bd904 | 2030-08-08T05:15:00Z | aaa | | 3 | |'
                                        onChange={changeInputHandler}
                                    />
                                </label>
                                <button
                                    className="buttonStyle"
                                    type="button"
                                    onClick={() => {
                                        setTxHash('');
                                        setTransactionError('');
                                        const tx = setValue(
                                            connection,
                                            account,
                                            useModuleSchema,
                                            isPayable,
                                            writeDropDown,
                                            input,
                                            cCDAmount
                                        );
                                        tx.then(setTxHash).catch((err: Error) =>
                                            setTransactionError((err as Error).message)
                                        );
                                    }}
                                >
                                    Set {writeDropDown} value
                                </button>
                                <div className="dashedLine" />
                                <div>Testing return value deserialization of functions:</div>
                                <div className="containerSpaceBetween">
                                    <div>Use module schema</div>
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
                                    <div>Use parameter schema</div>
                                </div>
                                <br />
                                <div>Select function:</div>
                                <br />
                                <div className="containerSpaceBetween">
                                    <div />
                                    <select
                                        className="centerLargeText"
                                        name="read"
                                        id="read"
                                        onChange={changeReadDropDownHandler}
                                    >
                                        <option value="u8" selected>
                                            u8
                                        </option>
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
                                    <div />
                                </div>
                                <button
                                    className="buttonStyle"
                                    type="button"
                                    onClick={() => {
                                        setReturnValue('');
                                        setReturnValueError('');
                                        withJsonRpcClient(connection, (rpcClient) =>
                                            getValue(rpcClient, useModuleSchema, readDropDown)
                                        )
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
                                    Get {readDropDown} value
                                </button>
                                {returnValue !== '' && (
                                    <>
                                        <div>Your return value is:</div>
                                        <div>{returnValue}</div>
                                    </>
                                )}
                                {!returnValue && returnValueError && (
                                    <div style={{ color: 'red' }}>Error: {returnValueError}.</div>
                                )}
                                <br />
                                <div className="dashedLine" />
                                <div>Testing complex object as input parameter:</div>
                                <br />
                                <br />
                                <div className="containerSpaceBetween">
                                    <div>Use module schema</div>
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
                                    <div>Use parameter schema</div>
                                </div>
                                <div className="containerSpaceBetween">
                                    <div>Is payable</div>
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
                                    <div>Is not payable</div>
                                </div>
                                <label>
                                    <p>CCD (micro):</p>
                                    <input
                                        className="inputFieldStyle"
                                        id="CCDAmount"
                                        type="text"
                                        placeholder="0"
                                        onChange={changeCCDAmountHandler}
                                    />
                                </label>
                                <button
                                    className="buttonStyle"
                                    type="button"
                                    onClick={() => {
                                        setTxHash('');
                                        setTransactionError('');
                                        const tx = setObject(
                                            connection,
                                            account,
                                            useModuleSchema,
                                            isPayable,
                                            cCDAmount
                                        );
                                        tx.then(setTxHash).catch((err: Error) =>
                                            setTransactionError((err as Error).message)
                                        );
                                    }}
                                >
                                    Set object
                                </button>
                                <br />
                                <div className="dashedLine" />
                                <div>Testing array as input parameter:</div>
                                <div className="containerSpaceBetween">
                                    <div>Use module schema</div>
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
                                    <div>Use parameter schema</div>
                                </div>
                                <div className="containerSpaceBetween">
                                    <div>Is payable</div>
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
                                    <div>Is not payable</div>
                                </div>
                                <br />
                                <br />
                                <label>
                                    <p>CCD (micro):</p>
                                    <input
                                        className="inputFieldStyle"
                                        id="CCDAmount"
                                        type="text"
                                        placeholder="0"
                                        onChange={changeCCDAmountHandler}
                                    />
                                </label>
                                <button
                                    className="buttonStyle"
                                    type="button"
                                    onClick={() => {
                                        setTxHash('');
                                        setTransactionError('');
                                        const tx = setArray(connection, account, useModuleSchema, isPayable, cCDAmount);
                                        tx.then(setTxHash).catch((err: Error) =>
                                            setTransactionError((err as Error).message)
                                        );
                                    }}
                                >
                                    Set Array
                                </button>
                                <br />
                                <div className="dashedLine" />
                                <div>Testing calling a function that calls another smart contract successfully:</div>
                                <button
                                    className="buttonStyle"
                                    type="button"
                                    onClick={() => {
                                        setTxHash('');
                                        setTransactionError('');
                                        const tx = internalCallSuccess(connection, account);
                                        tx.then(setTxHash).catch((err: Error) =>
                                            setTransactionError((err as Error).message)
                                        );
                                    }}
                                >
                                    Success (internal call to smart contract)
                                </button>
                                <br />
                                <div className="dashedLine" />
                                <div>Testing calling a function that reverts due to the smart contract logic:</div>
                                <button
                                    className="buttonStyle"
                                    type="button"
                                    onClick={() => {
                                        setTxHash('');
                                        setTransactionError('');
                                        const tx = reverts(connection, account);
                                        tx.then(setTxHash).catch((err: Error) =>
                                            setTransactionError((err as Error).message)
                                        );
                                    }}
                                >
                                    Revert
                                </button>
                                <br />
                                <div className="dashedLine" />
                                <div>Testing calling a function that reverts due to an internal call that reverts:</div>
                                <button
                                    className="buttonStyle"
                                    type="button"
                                    onClick={() => {
                                        setTxHash('');
                                        setTransactionError('');
                                        const tx = internalCallReverts(connection, account);
                                        tx.then(setTxHash).catch((err: Error) =>
                                            setTransactionError((err as Error).message)
                                        );
                                    }}
                                >
                                    Revert (internal call reverts)
                                </button>
                                <br />
                                <div className="dashedLine" />
                                <div>Testing calling a not existing entrypoint:</div>
                                <button
                                    className="buttonStyle"
                                    type="button"
                                    onClick={() => {
                                        setTxHash('');
                                        setTransactionError('');
                                        const tx = notExistingEntrypoint(connection, account);
                                        tx.then(setTxHash).catch((err: Error) =>
                                            setTransactionError((err as Error).message)
                                        );
                                    }}
                                >
                                    Not existing entrypoint (tx reverts)
                                </button>
                                <br />
                                <div className="dashedLine" />
                                <div>Testing simple CCD transfer:</div>
                                <label>
                                    <p>CCD (micro):</p>
                                    <input
                                        className="inputFieldStyle"
                                        id="CCDAmount"
                                        type="text"
                                        placeholder="0"
                                        onChange={changeCCDAmountHandler}
                                    />
                                </label>
                                <label>
                                    <p>To account:</p>
                                    <input
                                        className="inputFieldStyle"
                                        id="toAccount"
                                        type="text"
                                        placeholder="4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt"
                                        onChange={changeToAccountHandler}
                                    />
                                </label>
                                <button
                                    className="buttonStyle"
                                    type="button"
                                    onClick={() => {
                                        setTxHash('');
                                        setTransactionError('');
                                        const tx = simpleCCDTransfer(connection, account, toAccount, cCDAmount);
                                        tx.then(setTxHash).catch((err: Error) =>
                                            setTransactionError((err as Error).message)
                                        );
                                    }}
                                >
                                    Send simple CCD transfer
                                </button>
                                <br />
                                <div className="dashedLine" />
                                <div>Testing simple CCD transfer to non exising account address:</div>
                                <button
                                    className="buttonStyle"
                                    type="button"
                                    onClick={() => {
                                        setTxHash('');
                                        setTransactionError('');
                                        const tx = simpleCCDTransferToNonExistingAccountAddress(connection, account);
                                        tx.then(setTxHash).catch((err: Error) =>
                                            setTransactionError((err as Error).message)
                                        );
                                    }}
                                >
                                    Send simple CCD transfer to non existing account address (reverts)
                                </button>
                                <br />
                                <div className="dashedLine" />
                                <div>Testing signing a string message with the wallet:</div>
                                <label>
                                    <p>Message to be signed:</p>
                                    <input
                                        className="inputFieldStyle"
                                        id="message"
                                        type="text"
                                        placeholder="My message"
                                        onChange={changeMessageHandler}
                                    />
                                </label>
                                <button
                                    className="buttonStyle"
                                    type="button"
                                    onClick={() => {
                                        setSigningError('');
                                        setSignature('');
                                        const promise = connection.signMessage(account, {
                                            type: 'StringMessage',
                                            value: message,
                                        });
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
                                        <div> Your generated signature is: </div>
                                        <div>{signature}</div>
                                    </>
                                )}
                                <br />
                                <div className="dashedLine" />
                                <div>Testing signing a byte message with the wallet:</div>
                                <button
                                    className="buttonStyle"
                                    type="button"
                                    onClick={() => {
                                        const signMessage = {
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
                                        };

                                        const serializedMessage = serializeTypeValue(
                                            signMessage,
                                            toBuffer(SET_OBJECT_PARAMETER_SCHEMA, 'base64')
                                        );
                                        setSigningError('');
                                        setByteSignature('');
                                        const promise = connection.signMessage(account, {
                                            type: 'BinaryMessage',
                                            value: serializedMessage,
                                            schema: {
                                                type: 'TypeSchema',
                                                value: toBuffer(SET_OBJECT_PARAMETER_SCHEMA, 'base64'),
                                            },
                                        });
                                        promise
                                            .then((permitSignature) => {
                                                setByteSignature(permitSignature[0][0]);
                                            })
                                            .catch((err: Error) => setSigningError((err as Error).message));
                                    }}
                                >
                                    Sign message
                                </button>
                                {signingError && <div style={{ color: 'red' }}>Error: {signingError}.</div>}
                                {byteSignature !== '' && (
                                    <>
                                        <div> Your generated signature is: </div>
                                        <div>{byteSignature}</div>
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
