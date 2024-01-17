export { default } from './App';

export enum Network {
    Testnet = 'Testnet',
    Mainnet = 'Mainnet',
}

export interface SubMenuProps {
    network: Network;
    goHome: () => void;
}

export enum AppErrorType {
    Connection = 'Connection',
    Query = 'Query',
    FileError = 'FileError',
    WrongNetwork = 'WrongNetwork',
    InvalidIdObject = 'InvalidIdObject',
    InvalidSeedphrase = 'InvalidSeedphrase',
    SeedphraseIdObjectMismatch = 'SeedphraseIdObjectMismatch',
    Internal = 'Internal',
    TooManyAccounts = 'TooManyAccounts',
}

export interface AppError {
    type: AppErrorType;
    message: string;
}
