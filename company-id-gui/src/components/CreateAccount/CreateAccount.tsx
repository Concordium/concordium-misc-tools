import { Button, Form, FormControl, InputGroup } from 'react-bootstrap';
import { SubMenuProps } from '../App';
import { FormEvent, useState } from 'react';

function CreateAccount({ goHome, network }: SubMenuProps) {
    const [seedphraseError, setSeedphraseError] = useState(null as string | null);
    const [idObjectError, setIdObjectError] = useState(null as string | null);

    const get_accounts = (e: FormEvent<HTMLFormElement>) => {
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
    };

    return (
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
                {seedphraseError && <FormControl.Feedback type="invalid">{seedphraseError}</FormControl.Feedback>}
            </InputGroup>
            <Form.Label htmlFor="id-object">Identity object file</Form.Label>
            <InputGroup className="mb-3">
                <Form.Control id="id-object" type="file" name="id-object" isInvalid={idObjectError !== null} required />
                {idObjectError && <FormControl.Feedback type="invalid">{idObjectError}</FormControl.Feedback>}
            </InputGroup>

            <div className="d-flex align-items-baseline">
                <Button variant="secondary" onClick={goHome}>
                    Back
                </Button>
                <Button type="submit" variant="primary" className="ms-3">
                    Get Accounts
                </Button>
                <span className="ms-auto">Connected to: {network}</span>
            </div>
        </Form>
    );
}

export default CreateAccount;
