import { FormEvent, useEffect, useState } from 'react';
import { Button, Card, CardBody, CardHeader, Form, FormControl, InputGroup, ProgressBar } from 'react-bootstrap';
import { invoke } from '@tauri-apps/api/tauri';
import { writeText } from '@tauri-apps/api/clipboard';
import { open } from '@tauri-apps/api/shell';
import { Network, SubMenuProps } from '../App';

enum Step {
    Info = 0,
    ShowSeedphrase = 33,
    ReenterSeedphrase = 67,
    SaveRequest = 100,
}

function RequestIdentity({ goHome, network }: SubMenuProps) {
    const [step, setStep] = useState(Step.Info);

    return (
        <div style={{ width: 600 }}>
            {step === Step.Info ? (
                <Info back={goHome} proceed={() => setStep(Step.ShowSeedphrase)} />
            ) : step === Step.ShowSeedphrase ? (
                <ShowSeedphrase back={() => setStep(Step.Info)} proceed={() => setStep(Step.ReenterSeedphrase)} />
            ) : step === Step.ReenterSeedphrase ? (
                <ReenterSeedphrase
                    back={() => setStep(Step.ShowSeedphrase)}
                    proceed={() => setStep(Step.SaveRequest)}
                />
            ) : (
                <SaveRequest network={network} back={() => setStep(Step.ReenterSeedphrase)} />
            )}
            <ProgressBar now={step} className="mt-3 mb-3" />
            <div className="text-start">Network: {network}</div>
        </div>
    );
}

interface NavigationProps {
    back: () => void;
    proceed: () => void;
}

function Info({ back, proceed }: NavigationProps) {
    return (
        <>
            <p>You will now be guided though the process of creating a company ID.</p>
            <div className="d-flex justify-content-between">
                <Button variant="secondary" onClick={back}>
                    Back
                </Button>
                <Button variant="primary" onClick={proceed}>
                    Proceed
                </Button>
            </div>
        </>
    );
}

function ShowSeedphrase({ back, proceed }: NavigationProps) {
    const [seedphrase, setSeedphrase] = useState('');
    useEffect(() => {
        invoke<string>('get_seedphrase')
            .then((res) => setSeedphrase(res))
            .catch(console.error);
    }, []);

    const [copyIcon, setCopyIcon] = useState('bi-clipboard');
    const copySeedphrase = () => {
        setCopyIcon('bi-clipboard-check');
        writeText(seedphrase)
            .then(() => {
                setTimeout(() => setCopyIcon('bi-clipboard'), 1000);
            })
            .catch(console.error);
    };

    return (
        <>
            <p className="text-start">
                <strong>Important:</strong> Here is your seedphrase. It is the key that grants access to the created
                account, so please write it down and store it in a safe place.
            </p>
            <Card className="mb-3 text-start">
                <CardHeader className="d-flex justify-content-between align-items-center">
                    Seedphrase
                    <Button variant="outline-secondary" onClick={copySeedphrase} disabled={seedphrase == ''}>
                        <i className={copyIcon} />
                    </Button>
                </CardHeader>
                <CardBody className="font-monospace">
                    <span>{seedphrase}</span>
                </CardBody>
            </Card>
            <div className="d-flex justify-content-between">
                <Button variant="secondary" onClick={back}>
                    Back
                </Button>
                <Button variant="primary" onClick={proceed} disabled={seedphrase == ''}>
                    Proceed
                </Button>
            </div>
        </>
    );
}

function ReenterSeedphrase({ back, proceed }: NavigationProps) {
    const [error, setError] = useState<string | null>(null);

    const handleSubmit = (e: FormEvent<HTMLFormElement>) => {
        e.preventDefault();
        const formData = new FormData(e.target as HTMLFormElement);
        const seedphrase = formData.get('seedphrase') as string;

        if (seedphrase === '') {
            setError('Please enter your seedphrase.');
            return;
        }

        invoke<boolean>('check_seedphrase', { seedphrase })
            .then((res) => {
                if (res) {
                    setError(null);
                    proceed();
                } else {
                    setError('The seedphrase you entered is incorrect.');
                }
            })
            .catch(console.error);
    };

    return (
        <Form noValidate className="text-start" onSubmit={handleSubmit}>
            <p>Reenter your seedphrase to confirm that you have written it down correctly.</p>
            <InputGroup className="mb-3">
                <Form.Control
                    className="font-monospace"
                    as="textarea"
                    rows={4}
                    name="seedphrase"
                    isInvalid={error !== null}
                    required
                />
                {error && <FormControl.Feedback type="invalid">{error}</FormControl.Feedback>}
            </InputGroup>
            <div className="d-flex justify-content-between">
                <Button variant="secondary" onClick={back}>
                    Back
                </Button>
                <Button variant="primary" type="submit">
                    Proceed
                </Button>
            </div>
        </Form>
    );
}

function SaveRequest({ back, network }: Omit<NavigationProps, 'proceed'> & { network: Network }) {
    const [isSaving, setIsSaving] = useState(false);
    const saveRequest = async () => {
        setIsSaving(true);
        try {
            await invoke('save_request_file', { net: network });
        } finally {
            setIsSaving(false);
        }
    };
    const openDocumentation = () => {
        open('https://developer.concordium.software/en/mainnet/net/guides/company-identities.html').catch(
            console.error,
        );
    };

    return (
        <>
            <p className="text-start">
                Save the <code>request.json</code> file to your computer by clicking the button below. Then, follow the
                guide in the{' '}
                {
                    // eslint-disable-next-line jsx-a11y/anchor-is-valid
                    <a href="#" onClick={openDocumentation}>
                        Concordium Documentation
                    </a>
                }{' '}
                for instructions on what to do with the file. After saving the file, you may close this program.
            </p>
            <div className="d-flex">
                <Button variant="secondary" onClick={back} disabled={isSaving}>
                    Back
                </Button>
                <div className="ms-auto d-flex align-items-center">
                    {isSaving && <i className="bi-arrow-repeat spinner me-2" />}
                    <Button variant="primary" onClick={saveRequest} disabled={isSaving}>
                        Generate request.json
                    </Button>{' '}
                </div>
            </div>
        </>
    );
}

export default RequestIdentity;
