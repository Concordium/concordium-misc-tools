/* eslint-disable no-console */
/* eslint-disable no-alert */

import React, { useEffect, useState, ChangeEvent } from 'react';
import Switch from 'react-switch';
import {
    toBuffer,
    JsonRpcClient,
    serializeTypeValue,
    deserializeTypeValue,
    AccountAddress
} from '@concordium/web-sdk';
import { withJsonRpcClient, WalletConnectionProps, useConnection, useConnect } from '@concordium/react-components';
import { version } from '../package.json';

import { set_value, set_object, set_array } from './utils';
import {
    TX_CONTRACT_NAME,
    TX_CONTRACT_INDEX,
    CONTRACT_SUB_INDEX,
    BROWSER_WALLET,
    VIEW_RETURN_VALUE_SCHEMA,
    WALLET_CONNECT,

    REFRESH_INTERVAL,
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

const ButtonStyleSelected = {
    color: 'white',
    borderRadius: 10,
    margin: '7px 0px 7px 0px',
    padding: '10px',
    width: '100%',
    border: '0px solid',
    backgroundColor: '#174039',
    cursor: 'pointer',
    fontWeight: 300,
    fontSize: '14px',
};

const ButtonStyleNotSelected = {
    color: 'white',
    borderRadius: 10,
    margin: '7px 0px 7px 0px',
    padding: '10px',
    width: '100%',
    border: '0px solid',
    backgroundColor: '#308274',
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

// async function calculateTransferMessage(nonce: string, tokenID: string, from: string, to: string) {
//     if (nonce === '') {
//         alert('Insert a nonce.');
//         return '';
//     }

//     // eslint-disable-next-line no-restricted-globals
//     if (isNaN(Number(nonce))) {
//         alert('Your nonce needs to be a number.');
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
//         alert('Insert an `from` address.');
//         return '';
//     }

//     if (from.length !== 50) {
//         alert('`From` address needs to have 50 digits.');
//         return '';
//     }

//     if (to === '') {
//         alert('Insert an `to` address.');
//         return '';
//     }

//     if (to.length !== 50) {
//         alert('`To` address needs to have 50 digits.');
//         return '';
//     }

//     const transfer =
//         [
//             {
//                 amount: '1',
//                 data: '',
//                 from: {
//                     Account: [from],
//                 },
//                 to: {
//                     Account: [to],
//                 },
//                 token_id: tokenID,
//             },
//         ]

//     const payload = serializeTypeValue(
//         transfer,
//         toBuffer(TRANSFER_SCHEMA, 'base64')
//     );

//     const message = {
//         contract_address: {
//             index: Number(SPONSORED_TX_CONTRACT_INDEX),
//             subindex: 0,
//         },
//         nonce: Number(nonce),
//         timestamp: EXPIRY_TIME_SIGNATURE,
//         entry_point: 'transfer',
//         payload: Array.from(payload),
//     };

//     const serializedMessage = serializeTypeValue(
//         message,
//         toBuffer(SERIALIZATION_HELPER_SCHEMA, 'base64')
//     );

//     return serializedMessage;
// }

// async function calculateUpdateOperatorMessage(nonce: string, operator: string, addOperator: boolean) {
//     if (nonce === '') {
//         alert('Insert a nonce.');
//         return '';
//     }

//     // eslint-disable-next-line no-restricted-globals
//     if (isNaN(Number(nonce))) {
//         alert('Your nonce needs to be a number.');
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

//     const operatorAction = addOperator
//         ? {
//             Add: [],
//         }
//         : {
//             Remove: [],
//         };

//     const updateOperator =
//         [
//             {
//                 operator: {
//                     Account: [operator],
//                 },
//                 update: operatorAction,
//             }
//         ]

//     const payload = serializeTypeValue(
//         updateOperator,
//         toBuffer(UPDATE_OPERATOR_SCHEMA, 'base64')
//     );

//     const message = {
//         contract_address: {
//             index: Number(SPONSORED_TX_CONTRACT_INDEX),
//             subindex: 0,
//         },
//         nonce: Number(nonce),
//         timestamp: EXPIRY_TIME_SIGNATURE,
//         entry_point: 'updateOperator',
//         payload: Array.from(payload),
//     };

//     const serializedMessage = serializeTypeValue(
//         message,
//         toBuffer(SERIALIZATION_HELPER_SCHEMA, 'base64')
//     );

//     return serializedMessage;
// }

// async function getPublicKey(rpcClient: JsonRpcClient, account: string) {
//     const res = await rpcClient.getAccountInfo(account);
//     const publicKey = res?.accountCredentials[0].value.contents.credentialPublicKeys.keys[0].verifyKey;

//     return publicKey;
// }

async function view(rpcClient: JsonRpcClient) {

    const res = await rpcClient.invokeContract({
        method: `${TX_CONTRACT_NAME}.view`,
        contract: { index: TX_CONTRACT_INDEX, subindex: CONTRACT_SUB_INDEX },
    });

    if (!res || res.tag === 'failure' || !res.returnValue) {
        throw new Error(
            `RPC call 'invokeContract' on method '${TX_CONTRACT_NAME}.view' of contract '${TX_CONTRACT_INDEX}' failed`
        );
    }

    // @ts-ignore
    const state = deserializeTypeValue
        (toBuffer(res.returnValue, 'hex'),
            toBuffer(VIEW_RETURN_VALUE_SCHEMA, 'base64')
        );

    if (state === undefined) {
        throw new Error(
            `Deserializing the returnValue from the '${TX_CONTRACT_NAME}.view' method of contract '${TX_CONTRACT_INDEX}' failed`
        );
    } else {
        return JSON.stringify(state);
    }
}

async function account_info(rpcClient: JsonRpcClient, account: string) {
    return await rpcClient.getAccountInfo(account)
}

async function smart_contract_info(rpcClient: JsonRpcClient) {
    return await rpcClient.getInstanceInfo({ index: TX_CONTRACT_INDEX, subindex: CONTRACT_SUB_INDEX })
}

// function clearInputFields() {
//     const operator = document.getElementById('operator') as HTMLTextAreaElement;
//     if (operator !== null) {
//         operator.value = '';
//     }

//     const from = document.getElementById('from') as HTMLTextAreaElement;
//     if (from !== null) {
//         from.value = '';
//     }

//     const to = document.getElementById('to') as HTMLTextAreaElement;
//     if (to !== null) {
//         to.value = '';
//     }

//     const tokenID = document.getElementById('tokenID') as HTMLTextAreaElement;
//     if (tokenID !== null) {
//         tokenID.value = '';
//     }

//     const nonce = document.getElementById('nonce') as HTMLTextAreaElement;
//     if (nonce !== null) {
//         nonce.value = '';
//     }

//     const signer = document.getElementById('signer') as HTMLTextAreaElement;
//     if (signer !== null) {
//         signer.value = '';
//     }
// }

export default function Transactions(props: WalletConnectionProps) {
    const { network, activeConnectorType, activeConnector, activeConnectorError, connectedAccounts, genesisHashes } =
        props;


    const { connection, setConnection, account, genesisHash } = useConnection(connectedAccounts, genesisHashes);
    const { connect, isConnecting, connectError } = useConnect(activeConnector, setConnection);

    const [publicKeyError, setPublicKeyError] = useState('');

    const [isPermitUpdateOperator, setPermitUpdateOperator] = useState<boolean>(true);

    const [publicKey, setPublicKey] = useState('');
    const [nextNonce, setNextNonce] = useState<number>(0);

    const [accountInfoPublicKey, setAccountInfoPublicKey] = useState('');
    const [operator, setOperator] = useState('');
    const [addOperator, setAddOperator] = useState<boolean>(true);
    const [tokenID, setTokenID] = useState('');
    const [to, setTo] = useState('');
    const [nonce, setNonce] = useState('');
    const [from, setFrom] = useState('');
    const [signer, setSigner] = useState('');
    const [record, setRecord] = useState('');

    const [accountBalance, setAccountBalance] = useState('');
    const [smartContractBalance, setSmartContractBalance] = useState('');

    const [cCDAmount, setCCDAmount] = useState('');
    const [input, setInput] = useState('');

    const [useModuleSchema, setUseModuleSchema] = useState(true);
    const [isPayable, setIsPayable] = useState(true);
    const [dropDown, setDropDown] = useState('u8');

    const [signature, setSignature] = useState('');
    const [signingError, setSigningError] = useState('');

    const changeOperatorHandler = (event: ChangeEvent) => {
        const target = event.target as HTMLTextAreaElement;
        setOperator(target.value);
    };

    const changeTokenIDHandler = (event: ChangeEvent) => {
        const target = event.target as HTMLTextAreaElement;
        setTokenID(target.value);
    };

    const changeToHandler = (event: ChangeEvent) => {
        const target = event.target as HTMLTextAreaElement;
        setTo(target.value);
    };

    const changeFromHandler = (event: ChangeEvent) => {
        const target = event.target as HTMLTextAreaElement;
        setFrom(target.value);
    };

    const changeInputHandler = (event: ChangeEvent) => {
        const target = event.target as HTMLTextAreaElement;
        setInput(target.value);
    };

    const changeCCDAmountHandler = (event: ChangeEvent) => {
        const target = event.target as HTMLTextAreaElement;
        setCCDAmount(target.value);
    };

    const changeDropDownHandler = (event: ChangeEvent) => {
        var e = (document.getElementById("function")) as HTMLSelectElement;
        var sel = e.selectedIndex;
        var value = e.options[sel].value;
        setDropDown(value);
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
    const [transactionError, setTransactionError] = useState('');

    const [isWaitingForTransaction, setWaitingForUser] = useState(false);
    return (
        <div>
            <h1 className="header">Wallet Connect / Browser Wallet Testing Bench</h1>
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
                                    <select className="centerLargeBlackText" name="function" id="function" onChange={changeDropDownHandler}>
                                        <option value="u8" selected>u8</option>
                                        <option value="u16">u16</option>
                                        <option value="Address">Address</option>
                                        <option value="ContractAddress">ContractAddress</option>
                                        <option value="AccountAddress">AccountAddress</option>
                                    </select>
                                    <div></div>
                                </div>
                                <label>
                                    <p className="centerLargeText">CCD Funds:</p>
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
                                        placeholder='5 | 15 | {"Contract":[{"index":3,"subindex":0}]} or {"Account":["4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt"]} | {"index":3,"subindex":0} | 4fUk1a1rjBzoPCCy6p92u5LT5vSw9o8GpjMiRHBbJUfmx51uvt'
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
                                        const tx = set_value(connection, account, useModuleSchema, isPayable, dropDown, input, cCDAmount);
                                        tx.then(setTxHash)
                                            .catch((err: Error) => setTransactionError((err as Error).message))
                                            .finally(() => setWaitingForUser(false));
                                    }}
                                >
                                    Set {dropDown} value
                                </button>
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
                                    <p className="centerLargeText">CCD Funds:</p>
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
                                    <p className="centerLargeText">CCD Funds:</p>
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
                                <br />
                            </>
                        )}
                        {/* <div className="containerSpaceBetween">
                            <button
                                style={!isRegisterPublicKeyPage ? ButtonStyleNotSelected : ButtonStyleSelected}
                                type="button"
                                onClick={() => {
                                    setIsRegisterPublicKeyPage(true);
                                    setSignature('');
                                    setSigningError('');
                                    setTokenID('');
                                    setFrom('');
                                    setTo('');
                                    setOperator('');
                                    setNonce('');
                                    setSigner('');
                                    setTransactionError('');
                                    setTxHash('');
                                    clearInputFields();
                                }}
                            >
                                Register Public Key
                            </button>
                            <Switch
                                onChange={() => {
                                    setIsRegisterPublicKeyPage(!isRegisterPublicKeyPage);
                                    setSignature('');
                                    setSigningError('');
                                    setTokenID('');
                                    setFrom('');
                                    setTo('');
                                    setOperator('');
                                    setNonce('');
                                    setSigner('');
                                    setTransactionError('');
                                    setTxHash('');
                                    clearInputFields();
                                }}
                                onColor="#308274"
                                offColor="#308274"
                                onHandleColor="#174039"
                                offHandleColor="#174039"
                                checked={!isRegisterPublicKeyPage}
                                checkedIcon={false}
                                uncheckedIcon={false}
                            />
                            <button
                                style={isRegisterPublicKeyPage ? ButtonStyleNotSelected : ButtonStyleSelected}
                                type="button"
                                onClick={() => {
                                    setIsRegisterPublicKeyPage(false);
                                    setSignature('');
                                    setSigningError('');
                                    setTokenID('');
                                    setFrom('');
                                    setTo('');
                                    setOperator('');
                                    setNonce('');
                                    setSigner('');
                                    setTransactionError('');
                                    setTxHash('');
                                    clearInputFields();
                                }}
                            >
                                Submit Sponsored Tx
                            </button>
                        </div> */}
                    </>
                )}
                {/* {genesisHash && genesisHash !== network.genesisHash && (
                    <p style={{ color: 'red' }}>
                        Unexpected genesis hash: Please ensure that your wallet is connected to network{' '}
                        <code>{network.name}</code>.
                    </p>
                )} */}
            </div>
            {/* {connection && isRegisterPublicKeyPage && account !== undefined && (
                <>
                    {!publicKey && (
                        <>
                            <button
                                style={ButtonStyle}
                                type="button"
                                onClick={() => {
                                    setTxHash('');
                                    setTransactionError('');
                                    setWaitingForUser(true);
                                    const tx = register(connection, account, accountInfoPublicKey);
                                    tx.then(setTxHash)
                                        .catch((err: Error) => setTransactionError((err as Error).message))
                                        .finally(() => setWaitingForUser(false));
                                }}
                            >
                                Register Your Public Key
                            </button>
                        </>
                    )}
                    <br />
                    {publicKey !== '' && (
                        <>
                            <div> Your registered public key is: </div>
                            <div className="loadingText">{publicKey}</div>
                            <div> Your next nonce is: </div>
                            <div className="loadingText">{nextNonce}</div>
                        </>
                    )}
                </>
            )} */}
            {/* {connection && !isRegisterPublicKeyPage && account !== undefined && (
                <>
                    <div className="containerSpaceBetween">
                        <p>Update operator via a sponsored transaction</p>
                        <Switch
                            onChange={() => {
                                setPermitUpdateOperator(!isPermitUpdateOperator);
                                setSignature('');
                                setSigningError('');
                                setTxHash('');
                                setTransactionError('');
                                setNonce('');
                                setSigner('');
                                setTokenID('');
                                setFrom('');
                                setTo('');
                                setOperator('');
                                clearInputFields();
                            }}
                            onColor="#308274"
                            offColor="#308274"
                            onHandleColor="#174039"
                            offHandleColor="#174039"
                            checked={!isPermitUpdateOperator}
                            checkedIcon={false}
                            uncheckedIcon={false}
                        />
                        <p>Transfer via a sponsored transaction</p>
                    </div>
                    {publicKey === '' && <div style={{ color: 'red' }}>Register a public key first.</div>}
                    {isPermitUpdateOperator && publicKey !== '' && (
                        <>
                            <label>
                                <p style={{ marginBottom: 0 }}>Operator Address:</p>
                                <input
                                    className="input"
                                    style={InputFieldStyle}
                                    id="operator"
                                    type="text"
                                    placeholder="4HoVMVsj6TwJr6B5krP5fW9qM4pbo6crVyrr7N95t2UQDrv1fq"
                                    onChange={changeOperatorHandler}
                                />
                            </label>
                            <div className="containerSpaceBetween">
                                <p>Add operator</p>
                                <Switch
                                    onChange={() => {
                                        setAddOperator(!addOperator);
                                    }}
                                    onColor="#308274"
                                    offColor="#308274"
                                    onHandleColor="#174039"
                                    offHandleColor="#174039"
                                    checked={!addOperator}
                                    checkedIcon={false}
                                    uncheckedIcon={false}
                                />
                                <p>Remove operator</p>
                            </div>
                        </>
                    )}
                    {!isPermitUpdateOperator && publicKey !== '' && (
                        <>
                            <div>Mint a token to your account first:</div>
                            <button
                                style={ButtonStyle}
                                type="button"
                                onClick={async () => {
                                    setTxHash('');
                                    setTransactionError('');
                                    setWaitingForUser(true);
                                    const tx = mint(connection, account);
                                    tx.then(setTxHash)
                                        .catch((err: Error) => setTransactionError((err as Error).message))
                                        .finally(() => setWaitingForUser(false));
                                }}
                            >
                                Mint an NFT token
                            </button>
                            <label>
                                <p style={{ marginBottom: 0 }}>Token ID:</p>
                                <input
                                    className="input"
                                    style={InputFieldStyle}
                                    id="tokenID"
                                    type="text"
                                    placeholder="00000006"
                                    onChange={changeTokenIDHandler}
                                />
                            </label>
                            <label>
                                <p style={{ marginBottom: 0 }}>From Address:</p>
                                <input
                                    className="input"
                                    style={InputFieldStyle}
                                    id="from"
                                    type="text"
                                    placeholder="4HoVMVsj6TwJr6B5krP5fW9qM4pbo6crVyrr7N95t2UQDrv1fq"
                                    onChange={changeFromHandler}
                                />
                            </label>
                            <label>
                                <p style={{ marginBottom: 0 }}>To Address:</p>
                                <input
                                    className="input"
                                    style={InputFieldStyle}
                                    id="to"
                                    type="text"
                                    placeholder="4HoVMVsj6TwJr6B5krP5fW9qM4pbo6crVyrr7N95t2UQDrv1fq"
                                    onChange={changeToHandler}
                                />
                            </label>
                        </>
                    )} */}
            {/* {publicKey !== '' && (
                        <>
                            <label>
                                <p style={{ marginBottom: 0 }}>Nonce:</p>
                                <input
                                    className="input"
                                    style={InputFieldStyle}
                                    id="nonce"
                                    type="text"
                                    placeholder={nextNonce.toString()}
                                    onChange={changeNonceHandler}
                                />
                            </label>
                            <button
                                style={ButtonStyle}
                                type="button"
                                onClick={async () => {
                                    setSigningError('');
                                    setSignature('');
                                    const serializedMessage = isPermitUpdateOperator
                                        ? await calculateUpdateOperatorMessage(nonce, operator, addOperator)
                                        : await calculateTransferMessage(nonce, tokenID, from, to);

                                    if (serializedMessage !== '') {
                                        const promise = connection.signMessage(account, {
                                            data: serializedMessage.toString('hex'),
                                            schema: SERIALIZATION_HELPER_SCHEMA,
                                        })
                                        promise
                                            .then((permitSignature) => {
                                                setSignature(permitSignature[0][0]);
                                            })
                                            .catch((err: Error) => setSigningError((err as Error).message));
                                    } else {
                                        setSigningError('Serialization Error');
                                    }
                                }}
                            >
                                Generate Signature
                            </button>
                            <br />
                            {signingError && <div style={{ color: 'red' }}>Error: {signingError}.</div>}
                            {signature !== '' && (
                                <>
                                    <div> Your generated signature is: </div>
                                    <div className="loadingText">{signature}</div>
                                </>
                            )}
                            <br />
                            {publicKey !== '' && (
                                <>
                                    <div> Your registered public key is: </div>
                                    <div className="loadingText">{publicKey}</div>
                                    <div> Your next nonce is: </div>
                                    <div className="loadingText">{nextNonce}</div>
                                </>
                            )}
                            <label>
                                <p style={{ marginBottom: 0 }}>Signer:</p>
                                <input
                                    className="input"
                                    style={InputFieldStyle}
                                    id="signer"
                                    type="text"
                                    placeholder="4HoVMVsj6TwJr6B5krP5fW9qM4pbo6crVyrr7N95t2UQDrv1fq"
                                    onChange={changeSignerHandler}
                                />
                            </label>
                            <button
                                style={signature === '' ? ButtonStyleDisabled : ButtonStyle}
                                disabled={signature === ''}
                                type="button"
                                onClick={async () => {
                                    setTxHash('');
                                    setTransactionError('');
                                    setWaitingForUser(true);

                                    const tx = isPermitUpdateOperator
                                        ? submitUpdateOperator(VERIFIER_URL,
                                            signer,
                                            nonce,
                                            signature,
                                            operator,
                                            addOperator
                                        )
                                        : submitTransfer(VERIFIER_URL,
                                            signer,
                                            nonce,
                                            signature,
                                            tokenID,
                                            from,
                                            to
                                        );

                                    tx.then((txHashReturned) => {
                                        setTxHash(txHashReturned.tx_hash);
                                        if (txHashReturned.tx_hash !== '') {
                                            setSignature('');
                                            setTokenID('');
                                            setFrom('');
                                            setTo('');
                                            setOperator('');
                                            setNonce('');
                                            setSigner('');
                                            clearInputFields();
                                        }
                                    })
                                        .catch((err: Error) => setTransactionError((err as Error).message))
                                        .finally(() => {
                                            setWaitingForUser(false);
                                        });

                                }}
                            >
                                Submit Sponsored Transaction
                            </button>
                        </>
                    )}
                </>
            )} */}
            {/* {!connection && (
                <button style={ButtonStyleDisabled} type="button" disabled>
                    Waiting for connection...
                </button>
            )} */}
            {/* {connection && account && (
                <p>
                    {isRegisterPublicKeyPage && !publicKey && (
                        <>
                            <div>Transaction status{txHash === '' ? '' : ' (May take a moment to finalize)'}</div>
                            {!txHash && transactionError && (
                                <div style={{ color: 'red' }}>Error: {transactionError}.</div>
                            )}
                            {!txHash && !transactionError && <div className="loadingText">None</div>}
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
                        </>
                    )}
                    {!isRegisterPublicKeyPage && publicKey && (
                        <>
                            <div>Transaction status{txHash === '' ? '' : ' (May take a moment to finalize)'}</div>
                            {!txHash && transactionError && (
                                <div style={{ color: 'red' }}>Error: {transactionError}.</div>
                            )}
                            {!txHash && !transactionError && <div className="loadingText">None</div>}
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
                        </>
                    )}
                    {publicKeyError && <div style={{ color: 'red' }}>Error: {publicKeyError}.</div>}
                </p>
            )} */}
            <div>
                <br />
                Version: {version}
                <br />
            </div>
        </div>
    );
}
