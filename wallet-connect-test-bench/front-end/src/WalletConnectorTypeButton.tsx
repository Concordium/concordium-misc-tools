/* eslint-disable @typescript-eslint/no-explicit-any */

import React, { useCallback } from 'react';
import {
    ConnectorType,
    useWalletConnectorSelector,
    WalletConnection,
    WalletConnectionProps,
} from '@concordium/react-components';

function connectorTypeStyle(isSelected: boolean, isConnected: boolean) {
    const style = {};
    if (isConnected) {
        style.backgroundColor = '#823030';
        style.border = '1px solid #520C0C';
    } else if (isSelected) {
        style.backgroundColor = '#174039';
        style.border = '1px solid #0c221f';
    }
    return style;
}

interface Props extends WalletConnectionProps {
    connectorType: ConnectorType;
    connectorName: string;
    setWaitingForUser: (v: boolean) => void;
    connection: WalletConnection | undefined;
}

export function WalletConnectionTypeButton(props: Props) {
    const { connectorType, connectorName, setWaitingForUser, connection } = props;
    const { isSelected, isConnected, isDisabled, select } = useWalletConnectorSelector(
        connectorType,
        connection,
        props
    );
    const onClick = useCallback(() => {
        setWaitingForUser(false);
        select();
    }, [select]);
    return (
        <button
            className="btn btn-primary"
            style={connectorTypeStyle(isSelected, isConnected)}
            disabled={isDisabled}
            type="button"
            onClick={onClick}
        >
            {isConnected ? `Disconnect ${connectorName}` : `Use ${connectorName}`}
        </button>
    );
}
