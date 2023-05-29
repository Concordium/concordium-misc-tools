use anyhow::{anyhow, Context};
use clap::Parser;
use concordium_rust_sdk::{
    cis4::{Cis4Contract, Cis4TransactionMetadata, CredentialInfo, CredentialType, MetadataUrl},
    common::types::TransactionTime,
    contract_client,
    id::{
        constants::ArCurve, curve_arithmetic::Curve, pedersen_commitment::VecCommitmentKey,
        types::Attribute,
    },
    smart_contracts::common::{self as concordium_std, Amount, Serial, Timestamp},
    types::{transactions::send::GivenEnergy, ContractAddress, WalletAccount},
    v2::{self, BlockIdentifier},
    web3id::{CommitmentInputs, CredentialHolderId, Request, Web3IdAttribute, Presentation},
};
use ed25519_dalek::{Keypair, Signer};
use key_derivation::{ConcordiumHdWallet, Net};
use rand::{Rng, thread_rng};
use std::{collections::BTreeMap, path::PathBuf};
use web3id_test::{CredentialSecrets, DataToSign, StoreParam, ViewResponse};

#[derive(Debug, clap::Subcommand)]
enum Action {
    #[clap(name = "register")]
    Register {
        #[clap(long = "registry")]
        /// Address of the registry contract.
        registry:   ContractAddress,
        /// Address of the storage contract.
        #[clap(long = "storage")]
        storage:    ContractAddress,
        #[clap(long = "attributes", help = "Path to the file with attributes.")]
        attributes: PathBuf,
        #[clap(long = "seed", help = "The path to the seed phrase.")]
        seed:       PathBuf,
        #[clap(long = "issuer", help = "The issuer's wallet.")]
        issuer:     PathBuf,
    },
    #[clap(name = "view", about = "View the credentials in a given contract.")]
    View {
        #[clap(long = "registry")]
        /// Address of the registry contract.
        registry: ContractAddress,
        #[clap(long = "seed", help = "The path to the seed phrase.")]
        seed:     PathBuf,
        #[clap(long = "index", help = "The index of the credential.")]
        index:    u32,
    },
    #[clap(name = "prove")]
    Prove {
        #[clap(
            long = "verifier",
            help = "URL of the verifier where to submit the presentation."
        )]
        verifier:  url::Url,
        #[clap(long = "index", help = "The index of the credential.")]
        index:     u32,
        #[clap(long = "storage")]
        storage:   ContractAddress,
        #[clap(long = "seed", help = "The path to the seed phrase.")]
        seed:      PathBuf,
        #[clap(long = "statement", help = "Path to the credential.")]
        statement: PathBuf,
    },
}

#[derive(serde::Deserialize)]
struct Credential {
    contract:   ContractAddress,
    holder:     CredentialHolderId,
    attributes: BTreeMap<u8, Web3IdAttribute>,
}

#[derive(clap::Parser, Debug)]
#[command(author, version, about)]
#[command(propagate_version = true)]
struct App {
    #[clap(
        long = "node",
        help = "GRPC interface of the node.",
        default_value = "http://localhost:20000",
        global = true
    )]
    endpoint: v2::Endpoint,
    #[command(subcommand)]
    action:   Action,
}

#[derive(Debug, Clone, Copy)]
enum CredentialStorageType {}

type CredentialStorageContract = contract_client::ContractClient<CredentialStorageType>;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let app: App = App::parse();
    // TODO: TLS
    let endpoint = app
        .endpoint
        .connect_timeout(std::time::Duration::from_secs(5));
    let mut client = v2::Client::new(endpoint)
        .await
        .context("Unable to connect to the node.")?;

    match app.action {
        Action::Prove {
            verifier,
            statement,
            index,
            seed,
            storage,
        } => {
            let wallet = std::fs::read_to_string(&seed).context("Unable to read seed phrase.")?;
            let wallet = ConcordiumHdWallet::from_seed_phrase(wallet.as_str(), Net::Testnet);

            let sec_key = wallet.get_verifiable_credential_signing_key(index)?;
            let pub_key = wallet.get_verifiable_credential_public_key(index)?;
            let enc_key = wallet.get_verifiable_credential_encryption_key(index)?;
            let mut storage_client = CredentialStorageContract::create(client.clone(), storage)
                .await
                .context("Unable to construct storage client.")?;

            let holder_id = CredentialHolderId::new(pub_key);

            let Some(resp) = storage_client
                .view::<Option<ViewResponse>>(
                    "view",
                    &holder_id,
                    BlockIdentifier::LastFinal,
                )
                .await? else {
                    anyhow::bail!("Unable to retrieve credential with index {index} from the storage contract.")
                };
            let data = resp.decrypt(pub_key.into(), enc_key)?;

            let mut registry = Cis4Contract::create(client.clone(), data.issuer).await?;

            let info = registry
                .credential_entry(holder_id, BlockIdentifier::LastFinal)
                .await?;

            let statement = serde_json::from_reader(
                std::fs::File::open(&statement).context("Unable to open statement.")?,
            )
            .context("Unable to parse statement.")?;

            let statement = concordium_rust_sdk::web3id::CredentialStatement::Web3Id::<
                ArCurve,
                Web3IdAttribute,
            > {
                ty: [
                    "VerifiableCredential".into(),
                    "ConcordiumVerifiableCredential".into(),
                    info.credential_info.credential_type.credential_type,
                ]
                .into_iter()
                .collect(),
                network: concordium_rust_sdk::web3id::did::Network::Testnet,
                contract: data.issuer,
                credential: holder_id,
                statement,
            };
            let request = Request {
                challenge:             thread_rng().gen::<[u8; 32]>().into(),
                credential_statements: vec![statement],
            };
            let gc = client
                .get_cryptographic_parameters(BlockIdentifier::LastFinal)
                .await?;

            let secrets = CommitmentInputs::Web3Issuer {
                issuance_date: info.credential_info.valid_from.try_into()?,
                signer:        &sec_key,
                values:        &data.values,
                randomness:    data.randomness,
            };

            let start = chrono::Utc::now();
            let proof = request
                .prove(&gc.response, std::iter::once(secrets))
                .context("Cannot produce proof.")?;
            let end = chrono::Utc::now();
            println!("Took {}ms to produce proof.", end.signed_duration_since(start).num_milliseconds());

            let network_client = reqwest::ClientBuilder::new()
                .connect_timeout(std::time::Duration::from_secs(5))
                .timeout(std::time::Duration::from_secs(10))
                .build()?;

            let presentation: Presentation<ArCurve, Web3IdAttribute> = serde_json::from_str(&serde_json::to_string(&proof)?)?;
            
            let start = chrono::Utc::now();
            let response = network_client.post(verifier).json(&proof).send().await?;
            let end = chrono::Utc::now();
            println!("Took {}ms to get proof verified.", end.signed_duration_since(start).num_milliseconds());

            if response.status().is_success() {
                let body: serde_json::Value = response.json().await?;
                println!("{}", serde_json::to_string_pretty(&body)?);
            } else {
                println!("Verification failed.");
            }
        }
        Action::View {
            registry,
            seed,
            index,
        } => {
            let bi = client.get_consensus_info().await?.last_finalized_block;
            let mut registry_contract = Cis4Contract::create(client.clone(), registry)
                .await
                .context("Unable to construct registry contract.")?;
            let wallet = std::fs::read_to_string(&seed).context("Unable to read seed phrase.")?;
            let wallet = ConcordiumHdWallet::from_seed_phrase(wallet.as_str(), Net::Testnet);
            let pk = wallet.get_verifiable_credential_public_key(index)?;
            let holder = CredentialHolderId::new(pk);
            let entry = registry_contract
                .credential_entry(holder, bi)
                .await
                .context("Unable to get credential entry")?;
            let status = registry_contract
                .credential_status(holder, bi)
                .await
                .context("Unable to get credential status")?;

            println!("Entry: {entry:#?}");

            println!("Status: {status:#?}");
        }
        Action::Register {
            registry,
            storage,
            attributes,
            seed,
            issuer,
        } => {
            let wallet = std::fs::read_to_string(&seed).context("Unable to read seed phrase.")?;
            let wallet = ConcordiumHdWallet::from_seed_phrase(wallet.as_str(), Net::Testnet);

            let mut storage_client = CredentialStorageContract::create(client.clone(), storage)
                .await
                .context("Unable to construct storage client.")?;

            let mut idx = 0;
            loop {
                let pk = wallet.get_verifiable_credential_public_key(idx)?;
                let resp = storage_client
                    .view::<Option<ViewResponse>>(
                        "view",
                        &CredentialHolderId::new(pk),
                        BlockIdentifier::LastFinal,
                    )
                    .await?;
                if resp.is_none() {
                    break;
                } else {
                    idx += 1;
                }
            }
            println!("Using index = {}", idx);
            let sec_key = wallet.get_verifiable_credential_signing_key(idx)?;
            let pub_key = wallet.get_verifiable_credential_public_key(idx)?;
            let enc_key = wallet.get_verifiable_credential_encryption_key(idx)?;

            let mut registry_contract = Cis4Contract::create(client.clone(), registry)
                .await
                .context("Unable to construct registry contract.")?;

            let values: BTreeMap<u8, Web3IdAttribute> =
                serde_json::from_reader(&std::fs::File::open(&attributes)?)
                    .context("Unable to read attributes.")?;

            let crypto_params = client
                .get_cryptographic_parameters(BlockIdentifier::LastFinal)
                .await?;
            let (&h, _, bases) = crypto_params.response.vector_commitment_base();
            let comm_key = VecCommitmentKey {
                gs: bases.copied().collect(),
                h,
            };
            let mut gapped_values = Vec::new();
            for (k, v) in values.iter() {
                for _ in gapped_values.len()..usize::from(*k) {
                    gapped_values.push(ArCurve::scalar_from_u64(0));
                }
                gapped_values.push(v.to_field_element());
            }
            let mut rng = rand::thread_rng();
            let (comm, randomness) = comm_key
                .commit(&gapped_values, &mut rng)
                .context("Unable to commit.")?;

            let cred_info = CredentialInfo {
                holder_id:        CredentialHolderId::new(pub_key),
                holder_revocable: true,
                commitment:       concordium_rust_sdk::common::to_bytes(&comm),
                valid_from:       Timestamp::from_timestamp_millis(
                    chrono::Utc::now().timestamp_millis() as u64,
                ),
                valid_until:      None,
                credential_type:  CredentialType {
                    credential_type: "MyCredential".into(), // TODO
                },
                metadata_url:     MetadataUrl::new("foo".into(), None)?,
            };
            let issuer =
                WalletAccount::from_json_file(&issuer).context("Unable to get issuer's wallet.")?;
            let mut metadata = Cis4TransactionMetadata {
                sender_address: issuer.address,
                nonce:          client
                    .get_next_account_sequence_number(&issuer.address)
                    .await?
                    .nonce,
                expiry:         TransactionTime::hours_after(2),
                energy:         GivenEnergy::Add(10_000.into()),
                amount:         Amount::zero(),
            };
            let nonce: [u8; 12] = rng.gen();

            let secrets = CredentialSecrets {
                randomness,
                values,
                issuer: registry,
            };
            let encrypted_secrets = secrets.encrypt(cred_info.holder_id, enc_key, nonce)?;
            let payload_to_sign = DataToSign {
                contract_address:     storage_client.address,
                encrypted_credential: concordium_std::to_bytes(&encrypted_secrets),
                version:              0,
                timestamp:            Timestamp::from_timestamp_millis(
                    metadata.expiry.seconds * 1000,
                ),
            };
            let kp = Keypair {
                secret: sec_key,
                public: pub_key,
            };
            let mut data_to_sign = b"WEB3ID:STORE".to_vec();
            payload_to_sign
                .serial(&mut data_to_sign)
                .map_err(|()| anyhow::anyhow!("Could not serialize"))?;
            let signature = kp.sign(&data_to_sign);

            let register_response = registry_contract
                .register_credential(&issuer, &metadata, &cred_info)
                .await
                .context("Unable to register.")?;

            metadata.nonce.next_mut();
            let store_response = storage_client
                .make_call::<_, anyhow::Error>(&issuer, &metadata, "store", &StoreParam {
                    public_key: cred_info.holder_id,
                    signature:  signature.to_bytes(),
                    data:       payload_to_sign,
                })
                .await
                .context("Unable to store")?;

            let result = client.wait_until_finalized(&register_response).await?;
            println!("Register response = {:?}", result);

            let result = client.wait_until_finalized(&store_response).await?;
            println!("Store response = {:?}", result);
        }
    }

    Ok(())
}
