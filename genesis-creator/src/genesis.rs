//! Genesis types — re-exported from the Concordium Rust SDK's `genesis` module.
//!
//! Only types actually referenced within genesis-creator's binary code are
//! re-exported here.
pub use concordium_rust_sdk::genesis::{
    GenesisAccount, GenesisAccountPublic, GenesisData, UpdateKeysCollectionSkeleton,
};
