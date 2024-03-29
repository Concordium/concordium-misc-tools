/* eslint-disable no-alert */
import React, { useEffect, useState, MouseEventHandler, ChangeEventHandler } from 'react';
import Select from 'react-select';
import {
    AtomicStatement,
    AttributeKey,
    AttributeKeyString,
    IdStatement,
    StatementTypes,
    RevealStatement,
    IdProofOutput,
    IdStatementBuilder,
    AttributesKeys,
    RangeStatement,
} from '@concordium/web-sdk';
import { BrowserWalletProvider, WalletConnectProvider, WalletProvider } from './wallet-connection';

function getVerifierURL(): string {
    return window.location.origin;
}

interface StatementProps {
    statement: IdStatement;
}

/**
 * Component to display the statement.
 */
function Statement({ statement }: StatementProps) {
    return (
        <>
            {' '}
            {statement.map((s) => {
                switch (s.type) {
                    case StatementTypes.RevealAttribute:
                        return (
                            <div className="m-3 p-4 border rounded d-flex align-items-center">
                                <img
                                    src="https://robohash.org/hicveldicta.png?size=50x50&set=set1"
                                    className="mr-2"
                                    alt="img"
                                />
                                <div className="">
                                    <p className="fw-bold mb-1">{'Reveal attribute'}</p>
                                    <p className="fw-normal mb-1">{s.attributeTag}</p>
                                </div>
                            </div>
                        );
                    case StatementTypes.AttributeInRange:
                        return (
                            <div className="m-3 p-4 border rounded d-flex align-items-center">
                                <img
                                    src="https://robohash.org/hicveldicta.png?size=50x50&set=set1"
                                    className="mr-2"
                                    alt="img"
                                />
                                <div className="">
                                    <p className="fw-bold mb-1">{'Attribute in range'}</p>
                                    <p className="fw-normal mb-1">{s.attributeTag}</p>
                                    <p className="fw-normal mb-1">
                                        {' '}
                                        {'Lower: '} {s.lower}
                                    </p>
                                    <p className="fw-normal mb-1">
                                        {' '}
                                        {'Upper: '} {s.upper}
                                    </p>
                                </div>
                            </div>
                        );
                    case StatementTypes.AttributeInSet:
                        return (
                            <div className="m-3 p-4 border rounded d-flex align-items-center">
                                <img
                                    src="https://robohash.org/hicveldicta.png?size=50x50&set=set1"
                                    className="mr-2"
                                    alt="img"
                                />
                                <div className="">
                                    <p className="fw-bold mb-1">{'Attribute in set'}</p>
                                    <p className="fw-normal mb-1">{s.attributeTag}</p>
                                    <p className="fw-normal mb-1">
                                        {' '}
                                        {'Set: '} {s.set.join(', ')}
                                    </p>
                                </div>
                            </div>
                        );
                    case StatementTypes.AttributeNotInSet:
                        return (
                            <div className="m-3 p-4 border rounded d-flex align-items-center">
                                <img
                                    src="https://robohash.org/hicveldicta.png?size=50x50&set=set1"
                                    className="mr-2"
                                    alt="img"
                                />
                                <div className="">
                                    <p className="fw-bold mb-1">{'Attribute not in set'}</p>
                                    <p className="fw-normal mb-1">{s.attributeTag}</p>
                                    <p className="fw-normal mb-1">
                                        {' '}
                                        {'Set: '} {s.set.join(', ')}
                                    </p>
                                </div>
                            </div>
                        );
                }
            })}{' '}
        </>
    );
}

interface RevealAttributeProps {
    setStatement: (ns: AtomicStatement) => void;
}

async function submitProof(
    statement: IdStatement,
    provider: WalletProvider,
    setMessages: (cbk: (oldMessages: string[]) => string[]) => void
) {
    const response = await fetch(`${getVerifierURL()}/inject`, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
        },
        body: JSON.stringify(statement),
    });
    if (response.ok) {
        const body: { statement: IdStatement; challenge: string } = await response.json();
        let proof: IdProofOutput;
        try {
            proof = await provider.requestIdProof(body.statement, body.challenge);
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
        } catch (err: any) {
            if (err instanceof Error) {
                setMessages((oldMessages) => [...oldMessages, `Could not get proof: ${err.message}`]);
            } else {
                console.log(err);
            }
            return;
        }
        const resp = await fetch(`${getVerifierURL()}/prove`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({ challenge: body.challenge, proof }),
        });
        if (resp.ok) {
            setMessages((oldMessages) => [...oldMessages, 'Proof OK']);
        } else {
            const body = await resp.json();
            setMessages((oldMessages) => [...oldMessages, `Proof not OK: (${resp.status}) ${body}`]);
        }
    } else {
        setMessages((oldMessages: string[]) => [...oldMessages, `Could not inject statement: ${response.statusText}`]);
    }
}

function SubmitProof({ statement, provider }: { statement: IdStatement; provider: WalletProvider }) {
    const [messages, setMessages] = useState<string[]>([]);

    const handleProve: MouseEventHandler<HTMLButtonElement> = () => submitProof(statement, provider, setMessages);

    return (
        <div>
            <div>
                <button onClick={handleProve} type="button" className="btn btn-primary">
                    {'Prove'}
                </button>
            </div>
            <div>
                <ol>
                    {' '}
                    {messages.map((m) => (
                        <li className="alert alert-success"> {m} </li>
                    ))}{' '}
                </ol>
            </div>
        </div>
    );
}

const options = Object.values(AttributeKeyString).map((ak) => {
    return { value: ak, label: ak };
});
const initialOption: { value: AttributeKey; label: AttributeKey } = { value: 'firstName', label: 'firstName' };

function RevealAttribute({ setStatement }: RevealAttributeProps) {
    const [selected, setSelected] = useState<AttributeKey>('firstName');

    const handleChange = (option: { value: AttributeKey; label: AttributeKey } | null) => {
        if (option === null) {
            return;
        }
        setSelected(option.value as AttributeKey);
    };

    const onClickAdd: MouseEventHandler<HTMLButtonElement> = () => {
        setStatement({
            type: StatementTypes.RevealAttribute,
            attributeTag: selected,
        } as RevealStatement);
    };

    return (
        <form>
            <div className="form-group border rounded border-primary">
                <label>{'Reveal attribute.'} </label>
                <Select
                    className="rounded my-1"
                    onChange={handleChange}
                    options={options}
                    defaultValue={initialOption}
                />
                <div>
                    {' '}
                    <button onClick={onClickAdd} type="button" className="btn btn-primary">
                        {'Add'}
                    </button>{' '}
                </div>
            </div>
        </form>
    );
}

interface ExtendStatementProps {
    setStatement: (cbk: (s: IdStatement) => IdStatement) => void;
}

function AgeInRange({ setStatement }: ExtendStatementProps) {
    const [lower, setLower] = useState<string>('18');
    const [upper, setUpper] = useState<string>('64');

    const onLowerChange: ChangeEventHandler<HTMLInputElement> = (e) => {
        setLower(e.target.value);
    };

    const onUpperChange: ChangeEventHandler<HTMLInputElement> = (e) => {
        setUpper(e.target.value);
    };

    const onClickAdd: MouseEventHandler<HTMLButtonElement> = () => {
        const builder = new IdStatementBuilder(false);
        // Since addAgeInRange does some arithmetic we need to parse inputs as integers
        // first. Otherwise we get unexpected behaviour.
        builder.addAgeInRange(parseInt(lower), parseInt(upper));
        setStatement((oldStatements) => oldStatements.concat(builder.getStatement()));
    };

    return (
        <form>
            <div className="form-group border rounded border-primary my-2">
                <label>{'Prove age in range'} </label> <br />
                {'Lower age: '}
                <input className="my-1" onChange={onLowerChange} value={lower} />
                <br />
                {'Upper age: '}
                <input className="my-1" onChange={onUpperChange} value={upper} />
                <button onClick={onClickAdd} type="button" className="btn btn-primary">
                    {'Add'}
                </button>
            </div>
        </form>
    );
}

interface AgeBoundProps extends ExtendStatementProps {
    younger: boolean;
}

function AgeBound({ younger, setStatement }: AgeBoundProps) {
    const [bound, setBound] = useState<string>('18');

    const onBoundChange: ChangeEventHandler<HTMLInputElement> = (e) => {
        setBound(e.target.value);
    };

    const onClickAdd: MouseEventHandler<HTMLButtonElement> = () => {
        const builder = new IdStatementBuilder(false);
        // since addMaximumage and addMinimumAge do some arithmetic with the
        // bound we have to parse it to avoid weird behaviour that results from
        // adding and subtracting numbers and strings
        if (younger) {
            builder.addMaximumAge(parseInt(bound));
        } else {
            builder.addMinimumAge(parseInt(bound));
        }
        setStatement((oldStatements) => oldStatements.concat(builder.getStatement()));
    };

    return (
        <form>
            <div className="form-group border rounded border-primary my-2">
                <label>{`Prove${younger ? ' younger ' : ' older '}than`} </label> <br />
                <input className="my-1" onChange={onBoundChange} value={bound} />
                <br />
                <button onClick={onClickAdd} type="button" className="btn btn-primary">
                    {'Add'}
                </button>
            </div>
        </form>
    );
}

function AttributeInRange({ setStatement }: RevealAttributeProps) {
    const [lower, setLower] = useState<string>('');
    const [upper, setUpper] = useState<string>('');

    const [selected, setSelected] = useState<AttributeKey>('firstName');

    const handleChange = (option: { value: AttributeKey; label: AttributeKey } | null) => {
        if (option === null) {
            return;
        }
        setSelected(option.value as AttributeKey);
    };

    const onClickAdd: MouseEventHandler<HTMLButtonElement> = () => {
        setStatement({
            type: StatementTypes.AttributeInRange,
            attributeTag: selected,
            lower,
            upper,
        } as RangeStatement);
    };

    const onLowerChange: ChangeEventHandler<HTMLInputElement> = (e) => {
        setLower(e.target.value);
    };

    const onUpperChange: ChangeEventHandler<HTMLInputElement> = (e) => {
        setUpper(e.target.value);
    };

    return (
        <form>
            <div className="form-group border rounded border-primary my-2">
                <label>{'Prove attribute in range'} </label> <br />
                <Select
                    className="rounded my-1"
                    onChange={handleChange}
                    options={options}
                    defaultValue={initialOption}
                />
                {'Lower bound: '}
                <input className="my-1" onChange={onLowerChange} value={lower} />
                <br />
                {'Upper bound: '}
                <input className="my-1" onChange={onUpperChange} value={upper} />
                <button onClick={onClickAdd} type="button" className="btn btn-primary">
                    {'Add'}
                </button>
            </div>
        </form>
    );
}

interface SetMembershipProps extends RevealAttributeProps {
    member: boolean;
}

function AttributeInSet({ member, setStatement }: SetMembershipProps) {
    const [set, setSet] = useState<string>('');

    const [selected, setSelected] = useState<AttributeKey>('firstName');

    const handleChange = (option: { value: AttributeKey; label: AttributeKey } | null) => {
        if (option === null) {
            return;
        }
        setSelected(option.value as AttributeKey);
    };

    const onClickAdd: MouseEventHandler<HTMLButtonElement> = () => {
        setStatement({
            type: member ? StatementTypes.AttributeInSet : StatementTypes.AttributeNotInSet,
            attributeTag: selected,
            set: set.split(',').map((s) => s.trim()),
        });
    };

    const onLowerChange: ChangeEventHandler<HTMLInputElement> = (e) => {
        setSet(e.target.value);
    };

    return (
        <form>
            <div className="form-group border rounded border-primary my-2">
                <label>{`Prove attribute${member ? ' ' : ' not '} in set`} </label> <br />
                <Select
                    className="rounded my-1"
                    onChange={handleChange}
                    options={options}
                    defaultValue={initialOption}
                />
                {'Set: '}
                <input className="my-1" onChange={onLowerChange} value={set} />
                <br />
                <button onClick={onClickAdd} type="button" className="btn btn-primary">
                    {'Add'}
                </button>
            </div>
        </form>
    );
}

function DocumentExpiryNoEarlier({ setStatement }: ExtendStatementProps) {
    const [lower, setLower] = useState<string>('20250505');

    const onLowerChange: ChangeEventHandler<HTMLInputElement> = (e) => {
        setLower(e.target.value);
    };

    const onClickAdd: MouseEventHandler<HTMLButtonElement> = () => {
        const builder = new IdStatementBuilder(false);
        builder.documentExpiryNoEarlierThan(lower);
        setStatement((oldStatements) => oldStatements.concat(builder.getStatement()));
    };

    return (
        <form>
            <div className="form-group border rounded border-primary my-2">
                <label>{'Prove doc expiry no earlier than'} </label> <br />
                <input className="my-1" onChange={onLowerChange} value={lower} />
                <button onClick={onClickAdd} type="button" className="btn btn-primary">
                    {'Add'}
                </button>
            </div>
        </form>
    );
}

function DocumentIssuerIn({ setStatement }: ExtendStatementProps) {
    const [set, setSet] = useState<string>('');

    const onSetChange: ChangeEventHandler<HTMLInputElement> = (e) => {
        setSet(e.target.value);
    };

    const onClickAdd: MouseEventHandler<HTMLButtonElement> = () => {
        const builder = new IdStatementBuilder(false);
        builder.addMembership(
            AttributesKeys.idDocIssuer,
            set.split(',').map((e) => e.trim())
        );
        setStatement((oldStatements) => oldStatements.concat(builder.getStatement()));
    };

    return (
        <form>
            <div className="form-group border rounded border-primary my-2">
                <label>{'Prove document issuer in'} </label> <br />
                <input className="my-1" onChange={onSetChange} value={set} />
                <button onClick={onClickAdd} type="button" className="btn btn-primary">
                    {'Add'}
                </button>
            </div>
        </form>
    );
}

interface ExtendSetStatementProps extends ExtendStatementProps {
    member: boolean;
    attribute: AttributesKeys;
}

function AttributeIn({ attribute, member, setStatement }: ExtendSetStatementProps) {
    const [set, setSet] = useState<string>('');

    const onSetChange: ChangeEventHandler<HTMLInputElement> = (e) => {
        setSet(e.target.value);
    };

    const onClickAdd: MouseEventHandler<HTMLButtonElement> = () => {
        const builder = new IdStatementBuilder(false);
        if (member) {
            builder.addMembership(
                attribute,
                set.split(',').map((e) => e.trim())
            );
        } else {
            builder.addNonMembership(
                attribute,
                set.split(',').map((e) => e.trim())
            );
        }

        setStatement((oldStatements) => oldStatements.concat(builder.getStatement()));
    };

    return (
        <form>
            <div className="form-group border rounded border-primary my-2">
                <label>{`Prove ${Object.values(AttributeKeyString)[attribute]}${member ? ' ' : ' not '}in`} </label>{' '}
                <br />
                <input className="my-1" onChange={onSetChange} value={set} />
                <button onClick={onClickAdd} type="button" className="btn btn-primary">
                    {'Add'}
                </button>
            </div>
        </form>
    );
}

interface SpecialSetProps extends ExtendStatementProps {
    // if nationality is set then produce statement about EU nationality
    // otherwise about EU residence
    nationality: boolean;
}

function EUAttributeIn({ nationality, setStatement }: SpecialSetProps) {
    const onClickAdd: MouseEventHandler<HTMLButtonElement> = () => {
        const builder = new IdStatementBuilder(false);
        if (nationality) {
            builder.addEUNationality();
        } else {
            builder.addEUResidency();
        }

        setStatement((oldStatements) => oldStatements.concat(builder.getStatement()));
    };

    return (
        <form>
            <div className="form-group border rounded border-primary my-2">
                <label>{`Prove ${nationality ? 'nationality in EU' : 'residence in EU'}`} </label> <br />
                <button onClick={onClickAdd} type="button" className="btn btn-primary">
                    {'Add'}
                </button>
            </div>
        </form>
    );
}

/**
 * The main component.
 */
export default function ProofExplorer() {
    const [account, setAccount] = useState<string>();
    const [provider, setProvider] = useState<WalletProvider>();

    useEffect(() => {
        if (provider !== undefined) {
            provider.on('accountChanged', setAccount);

            return () => {
                provider?.disconnect?.().then(() => provider.removeAllListeners());
            };
        }
    }, [provider]);

    const connectProvider = async (provider: WalletProvider) => {
        setProvider(provider);
        setAccount(await provider.connect());
    };

    const [statement, setStatement] = useState<IdStatement>([]);

    const addStatement = (a: AtomicStatement) => {
        setStatement((oldStatement) => [...(oldStatement as IdStatement), a]);
    };

    return (
        <main className="container">
            <nav className="navbar bg-black mb-3">
                <div className="container-fluid">
                    <a className="navbar-brand text-white" href="#">
                        {'Proof explorer'}
                    </a>
                </div>
            </nav>
            <div className="row">
                <div className="col-sm">
                    <div>
                        <button
                            className="btn btn-primary me-1"
                            onClick={async () => connectProvider(await BrowserWalletProvider.getInstance())}
                        >
                            Connect browser
                        </button>
                        <button
                            className="btn btn-primary"
                            onClick={async () => connectProvider(await WalletConnectProvider.getInstance())}
                        >
                            Connect mobile
                        </button>
                    </div>
                    <p> Connected account: {account} </p>
                    {provider !== undefined && <SubmitProof statement={statement} provider={provider} />}
                </div>
                <div className="col-sm">
                    <div>
                        <RevealAttribute setStatement={addStatement} />
                    </div>
                    <div>
                        <AgeBound younger={true} setStatement={setStatement} />
                    </div>
                    <div>
                        <AgeBound younger={false} setStatement={setStatement} />
                    </div>
                    <div>
                        <AgeInRange setStatement={setStatement} />
                    </div>
                    <div>
                        <DocumentExpiryNoEarlier setStatement={setStatement} />
                    </div>
                    <div>
                        <DocumentIssuerIn setStatement={setStatement} />
                    </div>
                    <div>
                        <AttributeIn attribute={AttributesKeys.nationality} member={true} setStatement={setStatement} />
                    </div>
                    <div>
                        <AttributeIn
                            attribute={AttributesKeys.nationality}
                            member={false}
                            setStatement={setStatement}
                        />
                    </div>
                    <div>
                        <AttributeIn
                            attribute={AttributesKeys.countryOfResidence}
                            member={true}
                            setStatement={setStatement}
                        />
                    </div>
                    <div>
                        <AttributeIn
                            attribute={AttributesKeys.countryOfResidence}
                            member={false}
                            setStatement={setStatement}
                        />
                    </div>
                    <div>
                        <EUAttributeIn nationality={true} setStatement={setStatement} />
                    </div>
                    <div>
                        <EUAttributeIn nationality={false} setStatement={setStatement} />
                    </div>
                    <div>
                        <AttributeInRange setStatement={addStatement} />
                    </div>
                    <div>
                        <AttributeInSet member={true} setStatement={addStatement} />
                    </div>
                    <div>
                        <AttributeInSet member={false} setStatement={addStatement} />
                    </div>
                </div>
                <div className="col-sm">
                    <Statement statement={statement} />
                </div>
            </div>
        </main>
    );
}
