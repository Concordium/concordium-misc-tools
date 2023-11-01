import { useState } from 'react';
import { Button } from 'react-bootstrap';
import RequestIdentity from '../RequestIdentity/RequestIdentity';

enum MenuItem {
    RequestIdentity,
    CreateAccount,
    IdentityRecovery,
}

function App() {
    const [menuItem, setMenuItem] = useState(null as MenuItem | null);

    return (
        <div className="h-100 d-flex flex-column justify-content-center align-items-center">
            {menuItem === null ? (
                <>
                    <h1 className="mb-4">Concordium Company ID Tool</h1>
                    <div className="d-flex flex-column">
                        <Button
                            variant="primary"
                            onClick={() => setMenuItem(MenuItem.RequestIdentity)}
                            className="mb-3"
                        >
                            Request Identity
                        </Button>
                        <Button variant="primary" onClick={() => setMenuItem(MenuItem.CreateAccount)} className="mb-3">
                            Create Account
                        </Button>
                        <Button variant="secondary" onClick={() => setMenuItem(MenuItem.IdentityRecovery)}>
                            Identity Recovery
                        </Button>
                    </div>
                </>
            ) : menuItem === MenuItem.RequestIdentity ? (
                <RequestIdentity goBack={() => setMenuItem(null)} />
            ) : (
                <div></div>
            )}
        </div>
    );
}

export default App;
