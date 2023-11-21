import { Button, Form, InputGroup, ListGroup } from 'react-bootstrap';
import { AppError, AppErrorType, SubMenuProps } from '../App';
import { useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { open } from '@tauri-apps/api/shell';

interface Account {
    idIndex: number;
    accIndex: number;
    address: string;
}

function IdentityRecovery({ goHome, network }: SubMenuProps) {
    const [accountList, setAccountList] = useState(null as Account[] | null);
    const [seedphraseError, setSeedphraseError] = useState(null as string | null);
    const [recoverError, setRecoverError] = useState(null as string | null);
    const [savingKeys, setSavingKeys] = useState(null as number | null);
    const [recoveringIdentities, setRecoveringIdentities] = useState(false);
    const [generatingRecoveryRequest, setGeneratingRecoveryRequest] = useState(false);
    const [seedphraseState, setSeedphraseState] = useState(null as string | null);

    const isWorking = recoveringIdentities || savingKeys !== null || generatingRecoveryRequest;

    const identities = useMemo(() => {
        if (accountList === null) return [0];

        const identities: number[] = [];
        for (const account of accountList ?? []) {
            if (!identities.includes(account.idIndex)) identities.push(account.idIndex);
        }
        for (let id = 0; id <= accountList.length; id++) {
            if (!identities.includes(id)) {
                identities.push(id);
                break;
            }
        }
        return identities;
    }, [accountList]);

    const [selectedIdentity, setSelectedIdentity] = useState(identities[0]);

    const recoverIdentities = async (e: React.FormEvent<HTMLFormElement>) => {
        e.preventDefault();
        const formData = new FormData(e.target as HTMLFormElement);
        const seedphrase = formData.get('seedphrase') as string;

        setSeedphraseError(null);
        if (seedphrase === '') {
            setSeedphraseError('Please enter your seedphrase.');
            return;
        }
        setSeedphraseState(seedphrase);

        setRecoverError(null);
        setRecoveringIdentities(true);
        try {
            const accounts = await invoke<Account[]>('recover_identities', { seedphrase });
            setAccountList(accounts);
            setRecoverError(null);
        } catch (e: unknown) {
            const err = e as AppError;
            if (err.type === AppErrorType.InvalidSeedphrase) setSeedphraseError(err.message);
            else setRecoverError(err.message);
        } finally {
            setRecoveringIdentities(false);
        }
    };

    const saveKeys = async (idx: number) => {
        const account = accountList?.[idx];
        if (!account) {
            console.error(`Account at index ${idx} not found`);
            setRecoverError('Something went wrong, try restarting the application.');
            return;
        }
        if (seedphraseState === null) {
            console.error('Seedphrase not set');
            setRecoverError('Something went wrong, try restarting the application.');
            return;
        }

        setSavingKeys(idx);
        try {
            await invoke('save_keys', { seedphrase: seedphraseState, account });
            setRecoverError(null);
        } catch (e: unknown) {
            setRecoverError((e as AppError).message);
        } finally {
            setSavingKeys(null);
        }
    };

    const generateRecoveryRequest = async () => {
        if (seedphraseState === null) {
            console.error('Seedphrase not set');
            setRecoverError('Something went wrong, try restarting the application.');
            return;
        }

        setGeneratingRecoveryRequest(true);
        try {
            await invoke('generate_recovery_request', { seedphrase: seedphraseState, idIndex: selectedIdentity });
            setRecoverError(null);
        } catch (e: unknown) {
            setRecoverError((e as AppError).message);
        } finally {
            setGeneratingRecoveryRequest(false);
        }
    };

    const openDocumentation = () => {
        open('https://developer.concordium.software/en/mainnet/net/guides/company-identities.html').catch(
            console.error,
        );
    };

    return accountList === null ? (
        <Form noValidate className="text-start" style={{ width: 700 }} onSubmit={recoverIdentities}>
            <p>This menu can recover credentials for the supplied identity. To begin, enter your seedphrase below.</p>
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

            {recoverError && <p className="text-danger">{recoverError}</p>}

            <div className="d-flex align-items-baseline">
                <Button variant="secondary" onClick={goHome} disabled={recoveringIdentities}>
                    Back
                </Button>
                <Button type="submit" variant="primary" className="ms-3" disabled={recoveringIdentities}>
                    Find identities
                </Button>
                {recoveringIdentities && <i className="bi-arrow-repeat spinner align-self-center ms-2" />}
                <div className="ms-auto">Connected to: {network}</div>
            </div>
        </Form>
    ) : (
        <div className="text-start" style={{ width: 700 }}>
            {accountList.length === 0 ? (
                <p>
                    There are currently no accounts associated with the supplied identity. Press the button below to
                    create a new account.
                </p>
            ) : (
                <>
                    <p>
                        The below list of accounts are associated with the supplied identity. You can save keys for
                        individual accounts or generate a request to recover the identity object. See the{' '}
                        {
                            // eslint-disable-next-line jsx-a11y/anchor-is-valid
                            <a href="#" onClick={openDocumentation}>
                                Concordium Documentation
                            </a>
                        }{' '}
                        for instructions on what to do with the recovery request file generated.
                    </p>
                    <ListGroup className="mb-3">
                        {accountList.map((account, idx) => (
                            <ListGroup.Item
                                className="d-flex align-items-center"
                                key={Math.pow(2, account.idIndex * 3) * Math.pow(3, account.accIndex)}
                            >
                                {account.idIndex},{account.accIndex}: {account.address}
                                <div className="ms-auto d-flex">
                                    {savingKeys === idx && <i className="bi-arrow-repeat spinner align-self-center" />}
                                    <Button
                                        variant="secondary"
                                        className="ms-2"
                                        onClick={() => saveKeys(idx)}
                                        disabled={isWorking}
                                    >
                                        Save
                                    </Button>
                                </div>
                            </ListGroup.Item>
                        ))}
                    </ListGroup>
                </>
            )}

            {recoverError && <p className="mb-3 text-danger">{recoverError}</p>}

            {identities.length > 1 && (
                <InputGroup className="mb-3">
                    <InputGroup.Text>Identity to recover</InputGroup.Text>
                    <Form.Select
                        value={selectedIdentity}
                        onChange={(e) => setSelectedIdentity(parseInt(e.target.value))}
                    >
                        {identities.map((id) => (
                            <option key={id}>{id}</option>
                        ))}
                    </Form.Select>
                </InputGroup>
            )}

            <div className="d-flex align-items-center">
                <Button variant="secondary" onClick={() => setAccountList(null)} disabled={isWorking}>
                    Back
                </Button>
                <Button className="ms-3" variant="primary" onClick={generateRecoveryRequest} disabled={isWorking}>
                    Generate recovery request
                </Button>
                {generatingRecoveryRequest && <i className="bi-arrow-repeat spinner align-self-center ms-2" />}
                <div className="ms-auto">Connected to: {network}</div>
            </div>
        </div>
    );
}

export default IdentityRecovery;
