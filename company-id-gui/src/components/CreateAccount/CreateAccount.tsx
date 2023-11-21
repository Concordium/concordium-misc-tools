import { Button, Form, InputGroup, ListGroup, Table } from 'react-bootstrap';
import { AppError, AppErrorType, Network, SubMenuProps } from '../App';
import { FormEvent, useState } from 'react';
import { invoke } from '@tauri-apps/api/tauri';

interface IdentityData {
    accounts: Account[];
    attributes: [string, string][];
}

interface Account {
    accIndex: number;
    address: string;
}

function CreateAccount({ goHome, network }: SubMenuProps) {
    const [identityData, setIdentityData] = useState(null as IdentityData | null);
    const [seedphraseError, setSeedphraseError] = useState(null as string | null);
    const [idObjectError, setIdObjectError] = useState(null as string | null);
    const [getAccountsError, setGetAccountsError] = useState(null as string | null);
    const [idObjectState, setIDObjectState] = useState(null as string | null);
    const [seedphraseState, setSeedphraseState] = useState(null as string | null);
    const [gettingAccounts, setGettingAccounts] = useState(false);

    const get_accounts = async (e: FormEvent<HTMLFormElement>) => {
        e.preventDefault();
        const formData = new FormData(e.target as HTMLFormElement);
        const seedphrase = formData.get('seedphrase') as string;
        const idObject = formData.get('id-object') as File;

        setSeedphraseError(null);
        setIdObjectError(null);
        if (seedphrase === '') {
            setSeedphraseError('Please enter your seedphrase.');
        }
        if (idObject.name === '') {
            setIdObjectError('Please select an identity object.');
        }
        if (seedphrase === '' || idObject.name === '') {
            return;
        }

        setGettingAccounts(true);

        const idObjectText = await idObject.text();
        setIDObjectState(idObjectText);
        setSeedphraseState(seedphrase);
        try {
            const result = await invoke<IdentityData>('get_identity_data', { seedphrase, idObject: idObjectText });
            setIdentityData(result);
        } catch (e: unknown) {
            const err = e as AppError;
            console.log(err);
            if (err.type === AppErrorType.InvalidIdObject) setIdObjectError(err.message);
            else if (err.type == AppErrorType.InvalidSeedphrase) setSeedphraseError(err.message);
            else setGetAccountsError(err.message);
        } finally {
            setGettingAccounts(false);
        }
    };

    return identityData === null ? (
        <Form noValidate onSubmit={get_accounts} className="text-start" style={{ width: 700 }}>
            <p>
                This menu can be used to create a new account on the chain or regenerate the keys for an old account. In
                order to complete the account creation process you should have a 24 word seedphrase and an identity
                object from Notabene. If you do not have these, please follow the steps in the &ldquo;Request
                Identity&rdquo; menu.
            </p>
            <Form.Label htmlFor="seedphrase">Enter seedphrase</Form.Label>
            <InputGroup className="mb-3">
                <Form.Control
                    id="seedphrase"
                    className="font-monospace"
                    as="textarea"
                    rows={4}
                    name="seedphrase"
                    isInvalid={seedphraseError !== null}
                    required
                />
                {seedphraseError && <Form.Control.Feedback type="invalid">{seedphraseError}</Form.Control.Feedback>}
            </InputGroup>
            <Form.Label htmlFor="id-object">Identity object file</Form.Label>
            <InputGroup className="mb-3">
                <Form.Control
                    id="id-object"
                    type="file"
                    name="id-object"
                    isInvalid={idObjectError !== null}
                    accept=".json"
                    required
                />
                {idObjectError && <Form.Control.Feedback type="invalid">{idObjectError}</Form.Control.Feedback>}
            </InputGroup>

            {getAccountsError && <p className="mb-3 text-danger">{getAccountsError}</p>}

            <div className="d-flex align-items-baseline">
                <Button variant="secondary" onClick={goHome} disabled={gettingAccounts}>
                    Back
                </Button>
                <Button type="submit" variant="primary" className="ms-3" disabled={gettingAccounts}>
                    Get Accounts
                </Button>
                {gettingAccounts && <i className="bi-arrow-repeat spinner align-self-center ms-2" />}
                <span className="ms-auto">Connected to: {network}</span>
            </div>
        </Form>
    ) : (
        <Accounts
            goBack={() => setIdentityData(null)}
            network={network}
            seedphrase={seedphraseState!}
            idObject={idObjectState!}
            identityData={identityData}
            addAccount={(account) =>
                setIdentityData({ ...identityData, accounts: [...identityData.accounts, account] })
            }
        />
    );
}

interface AccountsProps {
    network: Network;
    seedphrase: string;
    idObject: string;
    identityData: IdentityData;
    goBack: () => void;
    addAccount: (account: Account) => void;
}

function Accounts({ network, seedphrase, idObject, identityData, goBack, addAccount }: AccountsProps) {
    const [creatingAccount, setCreatingAccount] = useState(false);
    const [createAccountError, setCreateAccountError] = useState(null as string | null);
    const [savingKeys, setSavingKeys] = useState(null as number | null);

    const createAccountButtonsDisabled = creatingAccount || savingKeys !== null;

    const createAccount = async () => {
        setCreatingAccount(true);
        let accountIndex = 0;
        for (const account of identityData.accounts) {
            if (account.accIndex > accountIndex) break;
            accountIndex++;
        }
        try {
            const account = await invoke<Account>('create_account', {
                idObject,
                seedphrase,
                accIndex: accountIndex,
            });
            setCreateAccountError(null);
            addAccount(account);
        } catch (e: unknown) {
            setCreateAccountError((e as AppError).message);
        } finally {
            setCreatingAccount(false);
        }
    };

    const saveKeys = async (account: Account) => {
        setSavingKeys(account.accIndex);
        try {
            await invoke('save_keys', { seedphrase, account });
        } catch (e: unknown) {
            setCreateAccountError((e as AppError).message);
        } finally {
            setSavingKeys(null);
        }
    };

    return (
        <div className="text-start" style={{ width: 700 }}>
            {identityData.attributes && (
                <>
                    <h2>Attributes</h2>
                    {identityData.attributes.length !== 0 && (
                        <Table bordered className="mb-3">
                            <tbody>
                                {identityData.attributes.map(([k, v]) => (
                                    <tr key={k}>
                                        <td>
                                            <strong>{k}</strong>
                                        </td>
                                        <td>{v}</td>
                                    </tr>
                                ))}
                            </tbody>
                        </Table>
                    )}
                    {identityData.attributes.length === 0 && <p> The identity object has no attributes. </p>}
                </>
            )}

            <h2>Accounts</h2>
            {identityData.accounts.length === 0 ? (
                <p className="mb-3">
                    There are currently no accounts associated with the supplied identity. Press the button below to
                    create a new account.
                </p>
            ) : (
                <>
                    <p className="mb-3">
                        The below list of accounts are associated with the supplied identity. You can save the keys for
                        an account by pressing the &ldquo;Save&rdquo; button next to it.
                    </p>
                    <ListGroup className="mb-3">
                        {identityData.accounts.map((account) => (
                            <ListGroup.Item className="d-flex align-items-center" key={account.accIndex}>
                                {account.accIndex}: {account.address}
                                <div className="ms-auto d-flex">
                                    {savingKeys === account.accIndex && (
                                        <i className="bi-arrow-repeat spinner align-self-center" />
                                    )}
                                    <Button
                                        variant="secondary"
                                        className="ms-2"
                                        onClick={() => saveKeys(account)}
                                        disabled={createAccountButtonsDisabled}
                                    >
                                        Save
                                    </Button>
                                </div>
                            </ListGroup.Item>
                        ))}
                    </ListGroup>
                </>
            )}

            {createAccountError && <p className="mb-3 text-danger">{createAccountError}</p>}

            <div className="d-flex align-items-center">
                <Button variant="secondary" onClick={goBack} disabled={createAccountButtonsDisabled}>
                    Back
                </Button>
                <Button
                    variant="primary"
                    className="ms-3"
                    onClick={createAccount}
                    disabled={createAccountButtonsDisabled}
                >
                    Create Account
                </Button>
                {creatingAccount && <i className="bi-arrow-repeat spinner ms-2" />}
                <span className="ms-auto">Connected to: {network}</span>
            </div>
        </div>
    );
}

export default CreateAccount;
