import { useState } from 'react';
import { Button, Form, InputGroup } from 'react-bootstrap';
import RequestIdentity from '../RequestIdentity/RequestIdentity';
import CreateAccount from '../CreateAccount/CreateAccount';
import { Network } from '.';
import { version } from '../../../package.json';
import { invoke } from '@tauri-apps/api/tauri';
import IdentityRecovery from '../IdentityRecovery/IdentityRecovery';

enum MenuItem {
    RequestIdentity,
    CreateAccount,
    IdentityRecovery,
}

function App() {
    const [menuItem, setMenuItem] = useState(null as MenuItem | null);
    const [network, setNetwork] = useState('Testnet' as Network);
    const [nodeURLError, setNodeURLError] = useState(null as string | null);
    const [nodeURL, setNodeURL] = useState(null as string | null);
    const [isConnecting, setIsConnecting] = useState(false);

    const defaultNodeURL =
        network === Network.Testnet
            ? 'http://node.testnet.concordium.com:20000'
            : 'https://grpc.mainnet.concordium.software:20000';

    const actualNodeURL = nodeURL ?? defaultNodeURL;

    const connectAndProceed = async (menuItem: MenuItem) => {
        if (actualNodeURL === null) {
            setNodeURLError('Please enter a node URL.');
            return;
        }
        setIsConnecting(true);
        try {
            await invoke('set_node_and_network', { endpoint: actualNodeURL, net: network });
            setMenuItem(menuItem);
        } catch (e) {
            setNodeURLError(e as string);
        } finally {
            setIsConnecting(false);
        }
    };

    return (
        <div className="d-flex flex-column justify-content-center align-items-center" style={{ minHeight: '100%' }}>
            <img src="/ccd-logo.svg" className="mb-4" width={100} height={100} alt="Concordium logo" />
            {menuItem === null ? (
                <>
                    <div className="d-flex align-items-baseline">
                        <h1 className="mb-4">Concordium Company ID Creator</h1>{' '}
                        <span className="ms-2">v. {version}</span>
                    </div>
                    <div className="mb-3" style={{ width: 500 }}>
                        <InputGroup className="mb-3">
                            <InputGroup.Text>Network</InputGroup.Text>
                            <Form.Select value={network} onChange={(e) => setNetwork(e.target.value as Network)}>
                                <option>{Network.Testnet}</option>
                                <option>{Network.Mainnet}</option>
                            </Form.Select>
                        </InputGroup>
                        <InputGroup hasValidation>
                            <InputGroup.Text>Node URL</InputGroup.Text>
                            <Form.Control
                                type="url"
                                name="node-url"
                                isInvalid={nodeURLError !== null}
                                value={actualNodeURL}
                                onChange={(e) => setNodeURL(e.target.value)}
                                required
                            />
                            {nodeURLError && (
                                <Form.Control.Feedback className="text-start" type="invalid">
                                    {nodeURLError}
                                </Form.Control.Feedback>
                            )}
                        </InputGroup>
                    </div>

                    {isConnecting && (
                        <div className="mb-3 d-flex">
                            <i className="bi-arrow-repeat spinner me-2" /> Connecting...
                        </div>
                    )}

                    <div className="d-flex flex-column">
                        <Button
                            variant="primary"
                            onClick={() => setMenuItem(MenuItem.RequestIdentity)}
                            className="mb-3"
                            disabled={isConnecting}
                        >
                            Request Identity
                        </Button>
                        <Button
                            variant="primary"
                            onClick={() => connectAndProceed(MenuItem.CreateAccount)}
                            className="mb-3"
                            disabled={isConnecting}
                        >
                            Create Account
                        </Button>
                        <Button
                            variant="secondary"
                            onClick={() => connectAndProceed(MenuItem.IdentityRecovery)}
                            disabled={isConnecting}
                        >
                            Identity Recovery
                        </Button>
                    </div>
                </>
            ) : menuItem === MenuItem.RequestIdentity ? (
                <RequestIdentity goHome={() => setMenuItem(null)} network={network} />
            ) : menuItem === MenuItem.CreateAccount ? (
                <CreateAccount goHome={() => setMenuItem(null)} network={network} />
            ) : (
                <IdentityRecovery goHome={() => setMenuItem(null)} network={network} />
            )}
        </div>
    );
}

export default App;
