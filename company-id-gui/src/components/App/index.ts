export { default } from './App';

export enum Network {
    Testnet = 'Testnet',
    Mainnet = 'Mainnet',
}

export interface SubMenuProps {
    network: Network;
    goHome: () => void;
}
