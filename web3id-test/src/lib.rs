use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm,
};
use concordium_rust_sdk::{
    base as concordium_base,
    base::common,
    id::{constants::ArCurve, pedersen_commitment::Randomness},
    smart_contracts::common::{self as concordium_std, Timestamp},
    types::ContractAddress,
    web3id::{CredentialHolderId, Web3IdAttribute},
};
use std::collections::BTreeMap;

/// Credential state that is used as the input parameter or the return value for
/// the `store/view` contract functions.
#[derive(concordium_std::Deserial, PartialEq)]
pub struct ViewResponse {
    /// Metadata associated with the credential.
    version:              u16,
    /// The `encrypted_credential`.
    encrypted_credential: Vec<u8>,
}

impl ViewResponse {
    pub fn decrypt(
        &self,
        pk: CredentialHolderId,
        key: [u8; 32],
    ) -> anyhow::Result<CredentialSecrets> {
        if self.version != 0 {
            anyhow::bail!("Unsupported version.");
        }
        let encrypted =
            concordium_std::from_bytes::<EncryptedCredentialSecrets>(&self.encrypted_credential)?;
        let cipher = Aes256Gcm::new(&key.into());
        let payload = aes_gcm::aead::Payload {
            msg: &encrypted.ciphertext,
            aad: pk.public_key.as_bytes(),
        };
        let decrypted = cipher.decrypt(&encrypted.nonce.into(), payload)?;
        let cs = common::from_bytes(&mut std::io::Cursor::new(decrypted))?;
        Ok(cs)
    }
}

/// The parameter type for the contract function `store`.
#[derive(concordium_std::Serialize, Debug)]
pub struct StoreParam {
    /// Public key that created the above signature.
    pub public_key: CredentialHolderId,
    /// Signature.
    pub signature:  [u8; ed25519_dalek::SIGNATURE_LENGTH],
    // The signed data.
    pub data:       DataToSign,
}

/// The parameter type for the contract function `serializationHelper`.
#[derive(concordium_std::Serialize, Debug)]
pub struct DataToSign {
    /// The contract_address that the signature is intended for.
    pub contract_address:     ContractAddress,
    /// The serialized encrypted_credential.
    #[concordium(size_length = 2)]
    pub encrypted_credential: Vec<u8>,
    /// Metadata associated with the credential.
    pub version:              u16,
    /// A timestamp to make signatures expire.
    pub timestamp:            Timestamp,
}

#[derive(common::Serial, common::Deserial, Debug, serde::Serialize, serde::Deserialize)]
pub struct CredentialSecrets {
    pub issuer: ContractAddress,
    /// The randomness from the commitment.
    pub randomness: Randomness<ArCurve>,
    /// The values that are committed to.
    pub values:     BTreeMap<u8, Web3IdAttribute>,
}

// TODO: Re-export Serial from concordium_base.
#[derive(concordium_std::Serial, concordium_std::Deserial)]
pub struct EncryptedCredentialSecrets {
    pub nonce:      [u8; 12],
    #[concordium(size_length = 2)]
    pub ciphertext: Vec<u8>,
}

impl CredentialSecrets {
    pub fn encrypt(
        &self,
        pk: CredentialHolderId,
        key: [u8; 32],
        nonce: [u8; 12],
    ) -> Result<EncryptedCredentialSecrets, aes_gcm::Error> {
        let cipher = Aes256Gcm::new(&key.into());
        let payload = aes_gcm::aead::Payload {
            msg: &common::to_bytes(self),
            aad: pk.public_key.as_bytes(),
        };
        let ciphertext = cipher.encrypt(&nonce.into(), payload)?;
        Ok(EncryptedCredentialSecrets { nonce, ciphertext })
    }
}
