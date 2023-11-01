import { useState } from 'react';
import { Button, Card, CardBody, CardHeader, FormControl, ProgressBar } from 'react-bootstrap';
import './RequestIdentity.scss';

enum Step {
    Info = 0,
    ShowSeedphrase = 33,
    ReenterSeedphrase = 67,
    SaveRequest = 100,
}

interface RequestIdentityProps {
    goBack: () => void;
}

function RequestIdentity({ goBack }: RequestIdentityProps) {
    const [step, setStep] = useState(Step.Info);

    const handleBack = () => {
        if (step === Step.Info) {
            goBack();
        } else if (step === Step.ShowSeedphrase) {
            setStep(Step.Info);
        } else if (step === Step.ReenterSeedphrase) {
            setStep(Step.ShowSeedphrase);
        } else {
            setStep(Step.ReenterSeedphrase);
        }
    };

    const handleProceed = () => {
        if (step === Step.Info) {
            setStep(Step.ShowSeedphrase);
        } else if (step === Step.ShowSeedphrase) {
            setStep(Step.ReenterSeedphrase);
        } else {
            setStep(Step.SaveRequest);
        }
    };

    return (
        <div>
            {step === Step.Info ? (
                <Info />
            ) : step === Step.ShowSeedphrase ? (
                <ShowSeedphrase />
            ) : step === Step.ReenterSeedphrase ? (
                <ReenterSeedphrase />
            ) : (
                <SaveRequest />
            )}

            <div className="d-flex justify-content-between">
                <Button variant="secondary" onClick={handleBack}>
                    Back
                </Button>
                {step === Step.SaveRequest ? (
                    <Button variant="primary">Save request.json</Button>
                ) : (
                    <Button variant="primary" onClick={handleProceed}>
                        Proceed
                    </Button>
                )}
            </div>
            <ProgressBar now={step} className="mt-3" />
        </div>
    );
}

function Info() {
    return <p>You will now be guided though the process of creating a company ID.</p>;
}

function ShowSeedphrase() {
    return (
        <>
            <p>Here is your seedphrase. Please write it down and store it in a safe place.</p>
            <Card className="mb-3 text-start">
                <CardHeader>Seedphrase</CardHeader>
                <CardBody className="font-monospace">
                    <span>insert seedphrase</span>
                </CardBody>
            </Card>
        </>
    );
}

function ReenterSeedphrase() {
    return (
        <>
            <p>Reenter your seedphrase to confirm that you have written it down correctly.</p>
            <Card className="mb-3 text-start">
                <CardHeader>Seedphrase</CardHeader>
                <CardBody className="font-monospace">
                    <FormControl as="textarea" rows={3} />
                </CardBody>
            </Card>
        </>
    );
}

function SaveRequest() {
    return (
        <p className="text-start">
            Save the request.json file to your computer. Then, follow the guide at ... for instructions on what to do
            with the file. Afterwards, you can close this program.
        </p>
    );
}

export default RequestIdentity;
