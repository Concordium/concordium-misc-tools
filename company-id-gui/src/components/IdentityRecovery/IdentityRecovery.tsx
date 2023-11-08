import { Button, Form, InputGroup, ListGroup } from 'react-bootstrap';
import { SubMenuProps } from '../App';
import { useState } from 'react';
import { invoke } from '@tauri-apps/api/tauri';

interface Account {
    idIndex: number;
    accIndex: number;
    address: string;
}

function IdentityRecovery({ goHome, network }: SubMenuProps) {
    const [accountList, setAccountList] = useState(null as Account[] | null);
    const [seedphraseError, setSeedphraseError] = useState(null as string | null);
    const [recoverError, setRecoverError] = useState(null as string | null);
    const [recoveringAccount, setRecoveringAccount] = useState(null as number | null);
    const [recoveringIdentities, setRecoveringIdentities] = useState(false);

    const recoverIdentities = async (e: React.FormEvent<HTMLFormElement>) => {
        e.preventDefault();
        const formData = new FormData(e.target as HTMLFormElement);
        const seedphrase = formData.get('seedphrase') as string;
        if (seedphrase === '') {
            setSeedphraseError('Please enter your seedphrase.');
            return;
        }

        setRecoverError(null);
        setRecoveringIdentities(true);
        try {
            const accounts = await invoke<Account[]>('recover_identities', { seedphrase });
            setAccountList(accounts);
        } catch (e: unknown) {
            const errString = e as string;
            if (errString.startsWith('Invalid seedphrase')) setSeedphraseError(errString);
            else setRecoverError(e as string);
        } finally {
            setRecoveringIdentities(false);
        }
    };

    const recoverAccount = (idx: number) => {
        setRecoveringAccount(idx);
    };

    return accountList === null ? (
        <Form noValidate className="text-start" style={{ width: 600 }} onSubmit={recoverIdentities}>
            <p className="mb-3">
                This menu can recover credentials for a company identity. To begin, enter your seedphrase below.
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

            {recoverError && <p className="mb-3 text-danger">{recoverError}</p>}

            <div className="d-flex align-items-baseline">
                <Button variant="secondary" onClick={goHome} disabled={recoveringIdentities}>
                    Back
                </Button>
                <Button type="submit" variant="primary" className="ms-3" disabled={recoveringIdentities}>
                    Recover Identities
                </Button>
                {recoveringIdentities && <i className="bi-arrow-repeat spinner align-self-center ms-2" />}
                <div className="ms-auto">Connected to: {network}</div>
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
                        {accountList.map((account, idx) => (
                            <ListGroup.Item key={Math.pow(2, account.idIndex * 3) * Math.pow(3, account.accIndex)}>
                                {account.idIndex},{account.accIndex}: {account.address}
                                {recoveringAccount === idx && <i className="bi-arrow-repeat spinner ms-2" />}
                                <Button onClick={() => recoverAccount(idx)}>Recover</Button>
                            </ListGroup.Item>
                        ))}
                    </ListGroup>
                </>
            )}

            {recoverError && <p className="mb-3 text-danger">{recoverError}</p>}

            <div className="d-flex align-items-center">
                <Button variant="secondary" onClick={() => setAccountList(null)} disabled={recoveringAccount !== null}>
                    Back
                </Button>
            </div>
        </div>
    );
}

export default IdentityRecovery;
