import { Button, Form, InputGroup, ListGroup } from 'react-bootstrap';
import { SubMenuProps } from '../App';
import { FormEvent, useState } from 'react';
import { invoke } from '@tauri-apps/api/tauri';

interface Account {
    index: number;
    address: string;
}

function CreateAccount({ goHome, network }: SubMenuProps) {
    const [accountList, setAccountList] = useState(null as Account[] | null);
    const [seedphraseError, setSeedphraseError] = useState(null as string | null);
    const [idObjectError, setIdObjectError] = useState(null as string | null);
    const [getAccountsError, setGetAccountsError] = useState(null as string | null);
    const [createAccountError, setCreateAccountError] = useState(null as string | null);
    const [idObjectState, setIDObjectState] = useState(null as string | null);
    const [seedphraseState, setSeedphraseState] = useState(null as string | null);
    const [gettingAccounts, setGettingAccounts] = useState(false);
    const [creatingAccount, setCreatingAccount] = useState(false);

    const get_accounts = async (e: FormEvent<HTMLFormElement>) => {
        e.preventDefault();
        const formData = new FormData(e.target as HTMLFormElement);
        const seedphrase = formData.get('seedphrase') as string;
        const idObject = formData.get('id-object') as File;
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
        console.log(idObjectText);
        setIDObjectState(idObjectText);
        setSeedphraseState(seedphrase);
        try {
            const result = await invoke<Account[]>('get_identity_accounts', { seedphrase, idObject: idObjectText });
            setAccountList(result);
        } catch (e: unknown) {
            const errString = e as string;
            if (errString.startsWith('Invalid identity object')) setIdObjectError(errString);
            else if (errString.startsWith('Invalid seedphrase')) setSeedphraseError(errString);
            else setGetAccountsError(errString);
        } finally {
            setGettingAccounts(false);
        }
    };

    const createAccount = async () => {
        if (accountList === null) {
            console.error('Account list is null');
            return;
        }

        setCreatingAccount(true);
        let accountIndex = 0;
        for (const account of accountList) {
            if (account.index > accountIndex) break;
            accountIndex++;
        }
        try {
            const account = await invoke<Account>('create_account', {
                idObject: idObjectState,
                seedphrase: seedphraseState,
                accIndex: accountIndex,
            });
            setCreateAccountError(null);
            setAccountList([...accountList, account]);
        } catch (e: unknown) {
            setCreateAccountError(e as string);
        } finally {
            setCreatingAccount(false);
        }
    };

    return accountList === null ? (
        <Form noValidate onSubmit={get_accounts} className="text-start" style={{ width: 600 }}>
            <p>
                This menu can be used to create an account on the chain. In order to complete the account creation
                process you should have a 24 word seedphrase and an identity object from Notabene. If you do not have
                these, please follow the steps in the &ldquo;Request Identity&rdquo; menu.
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
        <div className="text-start" style={{ width: 600 }}>
            {accountList.length === 0 ? (
                <p className="mb-3">
                    There are currently no accounts associated with the company identity. Press the button below to
                    create a new account.
                </p>
            ) : (
                <>
                    <p className="mb-3">The below list of accounts are associated with the company identity.</p>
                    <ListGroup className="mb-3">
                        {accountList.map((account) => (
                            <ListGroup.Item key={account.index}>
                                {account.index}: {account.address}
                            </ListGroup.Item>
                        ))}
                    </ListGroup>
                </>
            )}

            {createAccountError && <p className="mb-3 text-danger">{createAccountError}</p>}

            <div className="d-flex align-items-center">
                <Button variant="secondary" onClick={() => setAccountList(null)} disabled={creatingAccount}>
                    Back
                </Button>
                <Button variant="primary" className="ms-3" onClick={createAccount} disabled={creatingAccount}>
                    Create Account
                </Button>
                {creatingAccount && <i className="bi-arrow-repeat spinner ms-2" />}
            </div>
        </div>
    );
}

export default CreateAccount;
