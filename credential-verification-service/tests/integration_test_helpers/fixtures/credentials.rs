use concordium_rust_sdk::base::curve_arithmetic::{Curve, Value};
use concordium_rust_sdk::base::ed25519::SigningKey;
use concordium_rust_sdk::base::elgamal::PublicKey;
use concordium_rust_sdk::base::web3id::v1::{
    AccountCredentialVerificationMaterial, CredentialProofPrivateInputs,
    CredentialVerificationMaterial, OwnedAccountCredentialProofPrivateInputs,
    OwnedCredentialProofPrivateInputs, OwnedIdentityCredentialProofPrivateInputs,
};
use concordium_rust_sdk::base::{dodis_yampolskiy_prf, ps_sig};
use concordium_rust_sdk::id::account_holder::generate_pio_v1_with_rng;
use concordium_rust_sdk::id::constants::{ArCurve, IpPairing};
use concordium_rust_sdk::id::elgamal::SecretKey;
use concordium_rust_sdk::id::identity_provider;
use concordium_rust_sdk::id::secret_sharing::Threshold;
use concordium_rust_sdk::id::types::{
    AccCredentialInfo, ArIdentity, ArInfo, ArInfos, Attribute, AttributeList, AttributeTag,
    CredentialHolderInfo, Description, GlobalContext, IdCredentials, IdObjectUseData,
    IdentityObjectV1, IpContext, IpData, IpIdentity, IpInfo, PreIdentityObjectV1, YearMonth,
};
use concordium_rust_sdk::types::CredentialRegistrationID;
use concordium_rust_sdk::web3id::Web3IdAttribute;
use rand::{Rng, SeedableRng};
use std::collections::BTreeMap;

fn create_attribute_list(
    alist: BTreeMap<AttributeTag, Web3IdAttribute>,
) -> AttributeList<<ArCurve as Curve>::Scalar, Web3IdAttribute> {
    let valid_to = YearMonth::new(2050, 5).unwrap();
    let created_at = YearMonth::new(2020, 5).unwrap();
    AttributeList {
        valid_to,
        created_at,
        max_accounts: 237,
        alist,
        _phantom: Default::default(),
    }
}

pub struct IdentityCredentialsFixture {
    pub private_inputs: OwnedCredentialProofPrivateInputs<IpPairing, ArCurve, Web3IdAttribute>,
    pub issuer: IpIdentity,
}

impl IdentityCredentialsFixture {
    pub fn private_inputs(
        &self,
    ) -> CredentialProofPrivateInputs<'_, IpPairing, ArCurve, Web3IdAttribute> {
        self.private_inputs.borrow()
    }
}

pub fn identity_credentials_fixture(
    global_context: &GlobalContext<ArCurve>,
) -> IdentityCredentialsFixture {
    let attrs = super::statements_and_attributes().1;

    let IpData {
        public_ip_info: ip_info,
        ip_secret_key,
        ..
    } = ip();

    let (ars_infos, _ars_secret) = ars(global_context);
    let ars_infos = ArInfos {
        anonymity_revokers: ars_infos,
    };

    let id_object_use_data = id_use_data(&mut seed0());
    let (context, pio, _randomness) = pio_v1(
        &id_object_use_data,
        &ip_info,
        &ars_infos.anonymity_revokers,
        global_context,
    );
    let alist = create_attribute_list(attrs);
    let ip_sig = identity_provider::sign_identity_object_v1_with_rng(
        &pio,
        context.ip_info,
        &alist,
        &ip_secret_key,
        &mut seed0(),
    )
    .expect("sign credentials");

    let id_object = IdentityObjectV1 {
        pre_identity_object: pio,
        alist: alist.clone(),
        signature: ip_sig,
    };

    let commitment_inputs = OwnedCredentialProofPrivateInputs::Identity(Box::new(
        OwnedIdentityCredentialProofPrivateInputs {
            ip_info: ip_info.clone(),
            ars_infos: ars_infos.clone(),
            id_object,
            id_object_use_data,
        },
    ));

    IdentityCredentialsFixture {
        private_inputs: commitment_inputs,
        issuer: ip_info.ip_identity,
    }
}

pub struct AccountCredentialsFixture {
    pub private_inputs: OwnedCredentialProofPrivateInputs<IpPairing, ArCurve, Web3IdAttribute>,
    pub verification_material: CredentialVerificationMaterial<IpPairing, ArCurve>,
    pub cred_id: CredentialRegistrationID,
    pub issuer: IpIdentity,
}

impl AccountCredentialsFixture {
    pub fn private_inputs(
        &self,
    ) -> CredentialProofPrivateInputs<'_, IpPairing, ArCurve, Web3IdAttribute> {
        self.private_inputs.borrow()
    }
}

pub fn account_credentials_fixture(
    global_context: &GlobalContext<ArCurve>,
    index: u64,
) -> AccountCredentialsFixture {
    let attrs = super::statements_and_attributes().1;

    let cred_id_exp = ArCurve::generate_scalar(&mut seed(index));
    let cred_id = CredentialRegistrationID::from_exponent(global_context, cred_id_exp);

    let mut attr_rand = BTreeMap::new();
    let mut attr_cmm = BTreeMap::new();
    for (tag, attr) in &attrs {
        let attr_scalar = Value::<ArCurve>::new(attr.to_field_element());
        let (cmm, cmm_rand) = global_context
            .on_chain_commitment_key
            .commit(&attr_scalar, &mut seed0());
        attr_rand.insert(*tag, cmm_rand);
        attr_cmm.insert(*tag, cmm);
    }

    let issuer = IpIdentity::from(1u32);

    let commitment_inputs =
        OwnedCredentialProofPrivateInputs::Account(OwnedAccountCredentialProofPrivateInputs {
            attribute_values: attrs,
            attribute_randomness: attr_rand,
            issuer,
        });

    let credential_inputs =
        CredentialVerificationMaterial::Account(AccountCredentialVerificationMaterial {
            issuer,
            attribute_commitments: attr_cmm,
        });

    AccountCredentialsFixture {
        private_inputs: commitment_inputs,
        issuer,
        verification_material: credential_inputs,
        cred_id,
    }
}

pub fn seed0() -> rand::rngs::StdRng {
    seed(0)
}

pub fn seed(seed: u64) -> rand::rngs::StdRng {
    rand::rngs::StdRng::seed_from_u64(seed)
}

pub fn global_context() -> GlobalContext<ArCurve> {
    GlobalContext::generate("Test".into())
}

/// Create #num_ars anonymity revokers to be used by test
pub fn ars(
    global_context: &GlobalContext<ArCurve>,
) -> (
    BTreeMap<ArIdentity, ArInfo<ArCurve>>,
    BTreeMap<ArIdentity, SecretKey<ArCurve>>,
) {
    let ar_base = global_context.on_chain_commitment_key.g;
    let mut csprng = seed0();
    let mut ar_infos = BTreeMap::new();
    let mut ar_keys = BTreeMap::new();
    for i in 1..=NUM_ARS {
        let ar_id = ArIdentity::try_from(i as u32).unwrap();
        let ar_secret_key = SecretKey::generate(&ar_base, &mut csprng);
        let ar_public_key = PublicKey::from(&ar_secret_key);
        let ar_info = ArInfo::<ArCurve> {
            ar_identity: ar_id,
            ar_description: Description {
                name: format!("AnonymityRevoker{}", i),
                url: format!("AnonymityRevoker{}.com", i),
                description: format!("AnonymityRevoker{}", i),
            },
            ar_public_key,
        };
        let _ = ar_infos.insert(ar_id, ar_info);
        let _ = ar_keys.insert(ar_id, ar_secret_key);
    }
    (ar_infos, ar_keys)
}

const MAX_ATTRS: usize = 10;
const NUM_ARS: usize = 5;

/// Create identity provider with #num_ars ARs to be used by tests
pub fn ip() -> IpData<IpPairing> {
    let mut csprng = seed0();
    // Create key for IP long enough to encode the attributes and anonymity
    // revokers.
    let ps_len = 5 + NUM_ARS + MAX_ATTRS;
    let ip_secret_key = ps_sig::SecretKey::<IpPairing>::generate(ps_len, &mut csprng);
    let ip_verify_key = ps_sig::PublicKey::from(&ip_secret_key);
    let signing = SigningKey::generate(&mut csprng);
    let secret = signing.to_bytes();
    let ip_cdi_verify_key = signing.verifying_key();
    let ip_cdi_secret_key = secret;

    // Return IpData with public and private keys.
    IpData {
        public_ip_info: IpInfo {
            ip_identity: IpIdentity(1),
            ip_description: Description {
                name: "IP0".to_owned(),
                url: "IP0.com".to_owned(),
                description: "IP0".to_owned(),
            },
            ip_verify_key,
            ip_cdi_verify_key,
        },
        ip_secret_key,
        ip_cdi_secret_key,
    }
}

/// Create random IdObjectUseData to be used by tests
fn id_use_data<T: Rng>(csprng: &mut T) -> IdObjectUseData<IpPairing, ArCurve> {
    let aci = aci(csprng);
    let randomness = ps_sig::SigRetrievalRandomness::generate_non_zero(csprng);
    IdObjectUseData { aci, randomness }
}

/// Create random AccCredentialInfo (ACI) to be used by tests
fn aci<T: Rng>(csprng: &mut T) -> AccCredentialInfo<ArCurve> {
    let ah_info = CredentialHolderInfo::<ArCurve> {
        id_cred: IdCredentials::generate(csprng),
    };

    let prf_key = dodis_yampolskiy_prf::SecretKey::generate(csprng);
    AccCredentialInfo {
        cred_holder_info: ah_info,
        prf_key,
    }
}

fn pio_v1<'a>(
    id_use_data: &IdObjectUseData<IpPairing, ArCurve>,
    ip_info: &'a IpInfo<IpPairing>,
    ars_infos: &'a BTreeMap<ArIdentity, ArInfo<ArCurve>>,
    global_ctx: &'a GlobalContext<ArCurve>,
) -> (
    IpContext<'a, IpPairing, ArCurve>,
    PreIdentityObjectV1<IpPairing, ArCurve>,
    ps_sig::SigRetrievalRandomness<IpPairing>,
) {
    let mut csprng = seed0();

    // Create context with all anonymity revokers
    let context = IpContext::new(ip_info, ars_infos, global_ctx);

    // Select all ARs except last one
    let threshold = Threshold::try_from(NUM_ARS - 1)
        .unwrap_or(Threshold::try_new(1).expect("Threshold of 1 will never fail"));

    // Create and return PIO
    let (pio, randomness) = generate_pio_v1_with_rng(&context, threshold, id_use_data, &mut csprng)
        .expect("Generating the pre-identity object should succeed.");
    (context, pio, randomness)
}
