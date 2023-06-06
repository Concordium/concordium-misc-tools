/* eslint-disable no-console */
import React, { useEffect, useState, ChangeEvent, PropsWithChildren } from 'react';
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
    initializeWithoutAmountWithoutParameter,
    initializeWithAmount,
    initializeWithParameter,
    deploy,
    simpleCCDTransfer,
    simpleCCDTransferToNonExistingAccountAddress,
} from './writing_to_blockchain';

import {
    CONTRACT_INDEX,
    BROWSER_WALLET,
    WALLET_CONNECT,
    SET_OBJECT_PARAMETER_SCHEMA,
    REFRESH_INTERVAL,
} from './constants';

type TestBoxProps = PropsWithChildren<{
    header: string;
    note: string;
}>;

function TestBox({ header, children, note }: TestBoxProps) {
    return (
        <fieldset className="testBox">
            <legend>{header}</legend>
            <div className="testBoxFields">{children}</div>
            <br />
            <p className="note">{note}</p>
        </fieldset>
    );
}

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
        <main className="container">
            <div className="textCenter">
                Version: {version}
                <h1>Wallet Connect / Browser Wallet Testing Bench (With dApp Libraries)</h1>
                <WalletConnectionTypeButton
                    connectorType={BROWSER_WALLET}
                    connectorName="Browser Wallet"
                    setWaitingForUser={setWaitingForUser}
                    connection={connection}
                    {...props}
                />
                <WalletConnectionTypeButton
                    connectorType={WALLET_CONNECT}
                    connectorName="Wallet Connect"
                    setWaitingForUser={setWaitingForUser}
                    connection={connection}
                    {...props}
                />
                {activeConnectorError && (
                    <p className="alert alert-danger" role="alert">
                        Connector Error: {activeConnectorError}.
                    </p>
                )}
                {!activeConnectorError && !isWaitingForTransaction && activeConnectorType && !activeConnector && (
                    <p>
                        <i>Loading connector...</i>
                    </p>
                )}
                {connectError && (
                    <p className="alert alert-danger" role="alert">
                        Connect Error: {connectError}.
                    </p>
                )}
                {!connection && !isWaitingForTransaction && activeConnectorType && activeConnector && (
                    <p>
                        <button className="btn btn-primary me-1" type="button" onClick={connect}>
                            {isConnecting && 'Connecting...'}
                            {!isConnecting && activeConnectorType === BROWSER_WALLET && 'Connect Browser Wallet'}
                            {!isConnecting && activeConnectorType === WALLET_CONNECT && 'Connect Mobile Wallet'}
                        </button>
                    </p>
                )}
            </div>

            {account && (
                <div className="row">
                    {connection && account !== undefined && (
                        <>
                            <div className="col-lg-4">
                                <div className="sticky-top">
                                    <h5>This column includes various test scenarios that can be executed: </h5>
                                    <ul>
                                        <li>(IP) input parameter tests</li>
                                        <li>(RV) return value tests</li>
                                        <li>(TE) transaction execution tests</li>
                                        <li>(DI) deploying and initializing tests</li>
                                        <li>(ST) simple CCD transfer tests</li>
                                        <li>(SG) signature tests</li>
                                    </ul>
                                    <div className="inputFormatBox">
                                        <h3>Expected input parameter format:</h3>
                                        <ul>
                                            <li>
                                                <b>u8</b> (e.g. 5)
                                            </li>
                                            <li>
                                                <b>u16</b> (e.g. 15)
                                            </li>
                                            <li>
                                                <b>Address</b> (e.g
                                                &#123;&#34;Contract&#34;:[&#123;&#34;index&#34;:3,&#34;subindex&#34;:0&#125;]&#125;
                                                or
                                                &#123;&#34;Account&#34;:[&#34;4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt&#34;]&#125;
                                                )
                                            </li>
                                            <li>
                                                <b>ContractAddress</b> (e.g.
                                                &#123;&#34;index&#34;:3,&#34;subindex&#34;:0&#125;)
                                            </li>
                                            <li>
                                                <b>AccountAddress</b> (e.g.
                                                4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt)
                                            </li>
                                            <li>
                                                <b>Hash</b> (e.g.
                                                18ee24150dcb1d96752a4d6dd0f20dfd8ba8c38527e40aa8509b7adecf78f9c6)
                                            </li>
                                            <li>
                                                <b>Public key</b> (e.g.
                                                37a2a8e52efad975dbf6580e7734e4f249eaa5ea8a763e934a8671cd7e446499)
                                            </li>
                                            <li>
                                                <b>Signature</b> (e.g.
                                                632f567c9321405ce201a0a38615da41efe259ede154ff45ad96cdf860718e79bde07cff72c4d119c644552a8c7f0c413f5cf5390b0ea0458993d6d6374bd904)
                                            </li>
                                            <li>
                                                <b>Timestamp</b> (e.g. 2030-08-08T05:15:00Z)
                                            </li>
                                            <li>
                                                <b>String</b> (e.g. aaa)
                                            </li>
                                            <li>
                                                <b>Option (None)</b> (e.g. no input required)
                                            </li>
                                            <li>
                                                <b>Option (Some)</b> (e.g. 3)
                                            </li>
                                            <li>
                                                <b>Wrong schema</b> (e.g. 5)
                                            </li>
                                        </ul>
                                    </div>
                                </div>
                            </div>
                            <div className="col-lg-4">
                                <TestBox
                                    header="(IP) Testing simple input parameters"
                                    note="Expected result after pressing the button and confirming in wallet: The
                                        transaction hash or an error message should appear in the right column."
                                >
                                    <div className="switch-wrapper">
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
                                    <div className="switch-wrapper">
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
                                    <label className="field">
                                        Select function:
                                        <br />
                                        <select name="write" id="write" onChange={changeWriteDropDownHandler}>
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
                                            <option value="wrong_schema">
                                                Wrong schema (error should be returned)
                                            </option>
                                        </select>
                                    </label>
                                    <label className="field">
                                        CCD (micro):
                                        <br />
                                        <input
                                            className="inputFieldStyle"
                                            id="CCDAmount"
                                            type="text"
                                            placeholder="0"
                                            onChange={changeCCDAmountHandler}
                                        />
                                    </label>
                                    <label className="field">
                                        Input parameter:
                                        <br />
                                        <input
                                            className="inputFieldStyle"
                                            id="input"
                                            type="text"
                                            placeholder='5 | 15 | {"Contract":[{"index":3,"subindex":0}]} or {"Account":["4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt"]} | {"index":3,"subindex":0} | 4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt | 18ee24150dcb1d96752a4d6dd0f20dfd8ba8c38527e40aa8509b7adecf78f9c6 | 37a2a8e52efad975dbf6580e7734e4f249eaa5ea8a763e934a8671cd7e446499 | 632f567c9321405ce201a0a38615da41efe259ede154ff45ad96cdf860718e79bde07cff72c4d119c644552a8c7f0c413f5cf5390b0ea0458993d6d6374bd904 | 2030-08-08T05:15:00Z | aaa | | 3 | |'
                                            onChange={changeInputHandler}
                                        />
                                    </label>
                                    <br />
                                    <button
                                        className="btn btn-primary"
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
                                            tx.then(setTxHash).catch((err) =>
                                                setTransactionError((err as Error).message || (err as string))
                                            );
                                        }}
                                    >
                                        Set {writeDropDown} value
                                    </button>
                                </TestBox>
                                <TestBox
                                    header="(RV) Testing return value deserialization of functions"
                                    note="Expected result after pressing the button: The return value or an error message
                                        should appear in the above test unit."
                                >
                                    <div className="switch-wrapper">
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
                                    <label className="field">
                                        Select function:
                                        <br />
                                        <select name="read" id="read" onChange={changeReadDropDownHandler}>
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
                                            <option value="wrong_schema">
                                                Wrong schema (error should be returned)
                                            </option>
                                        </select>
                                    </label>
                                    <br />
                                    <button
                                        className="btn btn-primary"
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
                                        <div className="actionResultBox">
                                            <div>Your return value is:</div>
                                            <br />
                                            <div>{returnValue}</div>
                                        </div>
                                    )}
                                    {!returnValue && returnValueError && (
                                        <div className="alert alert-danger" role="alert">
                                            Error: {returnValueError}.
                                        </div>
                                    )}
                                </TestBox>
                                <TestBox
                                    header="(IP) Testing complex object as input parameter"
                                    note="Expected result after pressing the button and confirming in wallet: The
                                        transaction hash or an error message should appear in the right column."
                                >
                                    <div className="switch-wrapper">
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
                                    <div className="switch-wrapper">
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
                                    <label className="field">
                                        <p>CCD (micro):</p>
                                        <input
                                            className="inputFieldStyle"
                                            id="CCDAmount"
                                            type="text"
                                            placeholder="0"
                                            onChange={changeCCDAmountHandler}
                                        />
                                    </label>
                                    <br />
                                    <button
                                        className="btn btn-primary"
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
                                </TestBox>
                                <TestBox
                                    header="(IP) Testing array as input parameter"
                                    note="
                                                                                Expected result after pressing the button and confirming in wallet: The
                                        transaction hash or an error message should appear in the right column.
                                        "
                                >
                                    <div className="switch-wrapper">
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
                                    <div className="switch-wrapper">
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
                                    <label className="field">
                                        <p>CCD (micro):</p>
                                        <input
                                            className="inputFieldStyle"
                                            id="CCDAmount"
                                            type="text"
                                            placeholder="0"
                                            onChange={changeCCDAmountHandler}
                                        />
                                    </label>
                                    <br />
                                    <button
                                        className="btn btn-primary"
                                        type="button"
                                        onClick={() => {
                                            setTxHash('');
                                            setTransactionError('');
                                            const tx = setArray(
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
                                        Set Array
                                    </button>
                                </TestBox>
                                <TestBox
                                    header="
                                                                                    (TE) Testing calling a function that calls another smart contract
                                            successfully
                                        "
                                    note="
                                        Expected result after pressing the button and confirming in wallet: The
                                        transaction hash or an error message should appear in the right column.
                                        "
                                >
                                    <button
                                        className="btn btn-primary"
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
                                </TestBox>
                                <TestBox
                                    header="
                                            (TE) Testing calling a function that reverts due to the smart contract
                                            logic
                                        "
                                    note="
                                        Expected result after pressing the button and confirming in wallet: The
                                        transaction hash or an error message should appear in the right column.
                                        "
                                >
                                    <button
                                        className="btn btn-primary"
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
                                </TestBox>
                                <TestBox
                                    header="
                                            (TE) Testing calling a function that reverts due to an internal call that
                                            reverts
                                        "
                                    note="
                                        Expected result after pressing the button and confirming in wallet: The
                                        transaction hash or an error message should appear in the right column.
                                        "
                                >
                                    <button
                                        className="btn btn-primary"
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
                                </TestBox>
                                <TestBox
                                    header="(TE) Testing calling a not existing entrypoint"
                                    note="
                                        Expected result after pressing the button and confirming in wallet: The
                                        transaction hash or an error message should appear in the right column.
                                        "
                                >
                                    <button
                                        className="btn btn-primary"
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
                                </TestBox>
                                <TestBox
                                    header="(DI) Testing deploying a smart contract module"
                                    note="
                                        Expected result after pressing the button and confirming in wallet: The
                                        transaction hash or an error message should appear in the right column.
                                        "
                                >
                                    <button
                                        className="btn btn-primary"
                                        type="button"
                                        onClick={() => {
                                            setTxHash('');
                                            setTransactionError('');
                                            const tx = deploy(connection, account);
                                            tx.then(setTxHash).catch((err: Error) =>
                                                setTransactionError((err as Error).message)
                                            );
                                        }}
                                    >
                                        Deploy smart contract module
                                    </button>
                                </TestBox>
                                <TestBox
                                    header="(DI) Testing initializing a smart contract instance"
                                    note="
                                        Expected result after pressing the button and confirming in wallet: The
                                        transaction hash or an error message should appear in the right column.
                                        "
                                >
                                    <button
                                        className="btn btn-primary"
                                        type="button"
                                        onClick={() => {
                                            setTxHash('');
                                            setTransactionError('');
                                            const tx = initializeWithoutAmountWithoutParameter(connection, account);
                                            tx.then(setTxHash).catch((err: Error) =>
                                                setTransactionError((err as Error).message)
                                            );
                                        }}
                                    >
                                        Initialize smart contract instance (without parameter; without amount)
                                    </button>
                                </TestBox>
                                <TestBox
                                    header="(DI) Testing initializing a smart contract instance with parameter"
                                    note="
                                        Expected result after pressing the button and confirming in wallet: The
                                        transaction hash or an error message should appear in the right column.
                                        "
                                >
                                    <div className="switch-wrapper">
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
                                    <label className="field">
                                        Input parameter:
                                        <br />
                                        <input
                                            className="inputFieldStyle"
                                            id="input"
                                            type="text"
                                            placeholder="5"
                                            onChange={changeInputHandler}
                                        />
                                    </label>
                                    <button
                                        className="btn btn-primary"
                                        type="button"
                                        onClick={() => {
                                            setTxHash('');
                                            setTransactionError('');
                                            const tx = initializeWithParameter(
                                                connection,
                                                account,
                                                useModuleSchema,
                                                input
                                            );
                                            tx.then(setTxHash).catch((err: Error) =>
                                                setTransactionError((err as Error).message)
                                            );
                                        }}
                                    >
                                        Initialize smart contract instance with parameter (parameter should be of type
                                        `u16`)
                                    </button>
                                </TestBox>
                                <TestBox
                                    header="(DI) Testing initializing a smart contract instance with some CCD"
                                    note="
                                        Expected result after pressing the button and confirming in wallet: The
                                        transaction hash or an error message should appear in the right column.
                                        "
                                >
                                    <label className="field">
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
                                        className="btn btn-primary"
                                        type="button"
                                        onClick={() => {
                                            setTxHash('');
                                            setTransactionError('');
                                            const tx = initializeWithAmount(connection, account, cCDAmount);
                                            tx.then(setTxHash).catch((err: Error) =>
                                                setTransactionError((err as Error).message)
                                            );
                                        }}
                                    >
                                        Initialize smart contract instance with some CCD amount
                                    </button>
                                </TestBox>
                                <TestBox
                                    header="(ST) Testing simple CCD transfer"
                                    note="
                                        Expected result after pressing the button and confirming in wallet: The
                                        transaction hash or an error message should appear in the right column.
                                        "
                                >
                                    <label className="field">
                                        <p>CCD (micro):</p>
                                        <input
                                            className="inputFieldStyle"
                                            id="CCDAmount"
                                            type="text"
                                            placeholder="0"
                                            onChange={changeCCDAmountHandler}
                                        />
                                    </label>
                                    <label className="field">
                                        <p>To account:</p>
                                        <input
                                            className="inputFieldStyle"
                                            id="toAccount"
                                            type="text"
                                            placeholder="4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt"
                                            onChange={changeToAccountHandler}
                                        />
                                    </label>
                                    <br />
                                    <button
                                        className="btn btn-primary"
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
                                </TestBox>
                                <TestBox
                                    header="(ST) Testing simple CCD transfer to non exising account address"
                                    note="
                                        Expected result after pressing the button and confirming in wallet: The
                                        transaction hash or an error message should appear in the right column.
                                        "
                                >
                                    <button
                                        className="btn btn-primary"
                                        type="button"
                                        onClick={() => {
                                            setTxHash('');
                                            setTransactionError('');
                                            const tx = simpleCCDTransferToNonExistingAccountAddress(
                                                connection,
                                                account
                                            );
                                            tx.then(setTxHash).catch((err: Error) =>
                                                setTransactionError((err as Error).message)
                                            );
                                        }}
                                    >
                                        Send simple CCD transfer to non existing account address (reverts)
                                    </button>
                                </TestBox>
                                <TestBox
                                    header="(SG) Testing signing a string message with the wallet"
                                    note="
                                        Expected result after pressing button and confirming in wallet: A signature or
                                        an error message should appear in the above test unit.
                                        "
                                >
                                    <label className="field">
                                        <p>Message to be signed:</p>
                                        <input
                                            className="inputFieldStyle"
                                            id="message"
                                            type="text"
                                            placeholder="My message"
                                            onChange={changeMessageHandler}
                                        />
                                    </label>
                                    <br />
                                    <button
                                        className="btn btn-primary"
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
                                    {signingError && (
                                        <div className="alert alert-danger" role="alert">
                                            Error: {signingError}.
                                        </div>
                                    )}
                                    {signature !== '' && (
                                        <div className="actionResultBox">
                                            <div> Your generated signature is: </div>
                                            <br />
                                            <div>{signature}</div>
                                        </div>
                                    )}
                                </TestBox>
                                <TestBox
                                    header="(SG) Testing signing a byte message with the wallet"
                                    note="
                                        Expected result after pressing button and confirming in wallet: A signature or
                                        an error message should appear in the above test unit.
                                        "
                                >
                                    <button
                                        className="btn btn-primary"
                                        type="button"
                                        onClick={() => {
                                            const signMessage = {
                                                account_address_value:
                                                    '4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt',
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
                                                hash_value:
                                                    '37a2a8e52efad975dbf6580e7734e4f249eaa5ea8a763e934a8671cd7e446499',
                                                option_value: {
                                                    None: [],
                                                },
                                                public_key_value:
                                                    '37a2a8e52efad975dbf6580e7734e4f249eaa5ea8a763e934a8671cd7e446499',
                                                signature_value:
                                                    '632f567c9321405ce201a0a38615da41efe259ede154ff45ad96cdf860718e79bde07cff72c4d119c644552a8c7f0c413f5cf5390b0ea0458993d6d6374bd904',
                                                string_value: 'abc',
                                                timestamp_value: '2030-08-08T05:15:00Z',
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
                                    {signingError && (
                                        <div className="alert alert-danger" role="alert">
                                            Error: {signingError}.
                                        </div>
                                    )}
                                    {byteSignature !== '' && (
                                        <div className="actionResultBox">
                                            <div> Your generated signature is: </div>
                                            <br />
                                            <div>{byteSignature}</div>
                                        </div>
                                    )}
                                </TestBox>
                            </div>
                        </>
                    )}
                    <div className="col-lg-4">
                        <div className="sticky-top">
                            <h5>
                                This column refreshes every few seconds and displays balances, smart contract state,
                                transaction hashes, and error messages.
                            </h5>
                            <div className="label">Connected account:</div>
                            <div>
                                <a
                                    className="link"
                                    href={`https://testnet.ccdscan.io/?dcount=1&dentity=account&daddress=${account}`}
                                    target="_blank"
                                    rel="noreferrer"
                                >
                                    {account}
                                </a>
                            </div>
                            <br />
                            <div className="label">Your account balance:</div>
                            <div>{accountBalance.replace(/(\d)(?=(\d\d\d\d\d\d)+(?!\d))/g, '$1.')} CCD</div>
                            <br />
                            <div className="label">
                                Smart contract balance (index: {CONTRACT_INDEX.toString()}, subindex: 0):
                            </div>
                            <div>{smartContractBalance.replace(/(\d)(?=(\d\d\d\d\d\d)+(?!\d))/g, '$1.')} CCD</div>
                            <br />
                            <br />
                            <div className="label">
                                Error or Transaction status
                                {txHash === '' ? ':' : ' (May take a moment to finalize):'}
                            </div>
                            <br />
                            {!txHash && !transactionError && (
                                <div className="alert alert-danger" role="alert">
                                    IMPORTANT: After pressing a button on the left side that should send a transaction,
                                    the transaction hash or error returned by the wallet are displayed HERE.
                                </div>
                            )}
                            {!txHash && transactionError && (
                                <div className="alert alert-danger" role="alert">
                                    Error: {transactionError}.
                                </div>
                            )}
                            {viewError && (
                                <div className="alert alert-danger" role="alert">
                                    Error: {viewError}.
                                </div>
                            )}
                            {txHash && (
                                <a
                                    className="link"
                                    target="_blank"
                                    rel="noreferrer"
                                    href={`https://testnet.ccdscan.io/?dcount=1&dentity=transaction&dhash=${txHash}`}
                                >
                                    {txHash}
                                </a>
                            )}
                            <br />
                            <br />
                            <div className="label">Smart contract state:</div>
                            <pre className="largeText">{JSON.stringify(record, null, '\t')}</pre>
                        </div>
                    </div>
                </div>
            )}
        </main>
    );
}
