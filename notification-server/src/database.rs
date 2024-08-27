use crate::models::device::{Device, Preference};
use anyhow::Context;
use concordium_rust_sdk::common::types::AccountAddress;
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use lazy_static::lazy_static;
use std::{
    collections::{HashMap, HashSet},
    vec::IntoIter,
};
use concordium_rust_sdk::base::hashes::BlockHash;
use concordium_rust_sdk::types::AbsoluteBlockHeight;
use tokio_postgres::NoTls;
use tokio_postgres::types::ToSql;

#[derive(Clone, Debug)]
pub struct PreparedStatements {
    get_devices_from_account:   tokio_postgres::Statement,
    upsert_device:              tokio_postgres::Statement,
    get_latest_block_height:    tokio_postgres::Statement,
    insert_block:               tokio_postgres::Statement,
    pool:                       Pool,
}

impl PreparedStatements {
    async fn new(pool: Pool) -> anyhow::Result<Self> {
        let mut client = pool.get().await.context("Failed to get client")?;
        let transaction = client
            .transaction()
            .await
            .context("Failed to start a transaction")?;
        let get_devices_from_account = transaction
            .prepare(
                "SELECT device_id, preferences FROM account_device_mapping WHERE address = $1 \
                 LIMIT 1000",
            )
            .await
            .context("Failed to create account device mapping")?;
        let upsert_device = transaction
            .prepare(
                "INSERT INTO account_device_mapping (address, device_id, preferences) VALUES ($1, \
                 $2, $3) ON CONFLICT (address, device_id) DO UPDATE SET preferences = \
                 EXCLUDED.preferences;",
            )
            .await
            .context("Failed to create account device mapping")?;
        let get_latest_block_height = transaction
            .prepare(
                "SELECT blocks.height FROM blocks WHERE blocks.id = (SELECT MAX(blocks.id) FROM blocks);",
            )
            .await
            .context("Failed to create get latest block height")?;
        let insert_block = transaction
            .prepare(
                "INSERT INTO blocks (hash, height) VALUES ($1, $2);",
            )
            .await
            .context("Failed to create insert block")?;
        transaction
            .commit()
            .await
            .context("Failed to commit transaction")?;
        Ok(PreparedStatements {
            get_devices_from_account,
            upsert_device,
            get_latest_block_height,
            insert_block,
            pool,
        })
    }

    pub async fn get_processed_block_height(
        &self,
    ) -> anyhow::Result<Option<AbsoluteBlockHeight>> {
        let client = self.pool.get().await.context("Failed to get client")?;
        let row = client.query_opt(&self.get_latest_block_height, &[]).await?;
        row.map(|row| row.try_get::<_, i64>(0).context("Row did not have any returning values").map(|raw| (raw as u64).into())).transpose()
    }

    pub async fn get_devices_from_account(
        &self,
        account_address: &AccountAddress,
    ) -> anyhow::Result<Vec<Device>> {
        let client = self.pool.get().await.context("Failed to get client")?;
        let params: &[&(dyn tokio_postgres::types::ToSql + Sync)] = &[&account_address.0.as_ref()];
        let rows = client.query(&self.get_devices_from_account, params).await?;
        rows.iter()
            .map(|row| {
                let device_id = row.try_get::<_, String>(0)?;
                let preferences = bitmask_to_preferences(row.try_get::<_, i32>(1)?);
                Ok(Device::new(preferences, device_id))
            })
            .collect::<Result<Vec<Device>, _>>()
    }

    pub async fn upsert_subscription(
        &self,
        account_address: Vec<AccountAddress>,
        preferences: Vec<Preference>,
        device_id: &str,
    ) -> anyhow::Result<()> {
        let mut client = self.pool.get().await.context("Failed to get client")?;
        let preferences_mask = preferences_to_bitmask(preferences.into_iter());
        let transaction = client.transaction().await?;
        for account in account_address {
            let params: &[&(dyn ToSql + Sync)] =
                &[&account.0.as_ref(), &device_id, &preferences_mask];
            if let Err(e) = transaction.execute(&self.upsert_device, params).await {
                let _ = transaction.rollback().await;
                return Err(e.into());
            }
        }
        transaction.commit().await.map_err(Into::into)
    }

    pub async fn insert_block(
        &self,
        hash: &BlockHash,
        height: &AbsoluteBlockHeight,
    ) -> anyhow::Result<()> {
        let client = self.pool.get().await.context("Failed to get client")?;
        let params: &[&(dyn ToSql + Sync); 2] = &[&hash.as_ref(), &(height.height as i64)];
        client.execute(&self.insert_block, params).await?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct DatabaseConnection {
    pub prepared: PreparedStatements,
}

impl DatabaseConnection {
    pub async fn create(config: tokio_postgres::config::Config) -> anyhow::Result<Self> {
        let mgr_config = ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        };
        let mgr = Manager::from_config(config, NoTls, mgr_config);
        let pool = Pool::builder(mgr).max_size(16).build().unwrap();
        let prepared = PreparedStatements::new(pool).await?;
        Ok(DatabaseConnection { prepared })
    }
}

lazy_static! {
    static ref PREFERENCE_MAP: HashMap<Preference, i32> = vec![
        (Preference::CIS2Transaction, 1),
        (Preference::CCDTransaction, 2),
    ]
    .into_iter()
    .collect();
}

pub fn preferences_to_bitmask(preferences: IntoIter<Preference>) -> i32 {
    let unique_preferences: HashSet<Preference> = preferences.into_iter().collect();
    unique_preferences
        .iter()
        .fold(0, |acc, &pref| acc | PREFERENCE_MAP[&pref])
}

pub fn bitmask_to_preferences(bitmask: i32) -> HashSet<Preference> {
    PREFERENCE_MAP
        .iter()
        .filter_map(|(&key, &value)| {
            if bitmask & value != 0 {
                Some(key)
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use enum_iterator::all;

    use super::*;

    #[test]
    fn test_preference_map_coverage_and_uniqueness() {
        let expected_variants = all::<Preference>().collect::<HashSet<_>>();

        // Check for coverage
        for variant in &expected_variants {
            assert!(
                PREFERENCE_MAP.contains_key(variant),
                "PREFERENCE_MAP is missing the variant {:?}",
                variant
            );
        }

        // Check for uniqueness of indices
        let indices = PREFERENCE_MAP.values().cloned().collect::<HashSet<_>>();
        assert_eq!(
            indices.len(),
            PREFERENCE_MAP.len(),
            "Indices in PREFERENCE_MAP are not unique."
        );

        // Ensure all variants are accounted for
        assert_eq!(
            PREFERENCE_MAP.len(),
            expected_variants.len(),
            "PREFERENCE_MAP does not match the number of variants in Preference enum"
        );
    }

    #[test]
    fn test_preferences_to_bitmask_and_back() {
        let preferences = vec![Preference::CIS2Transaction, Preference::CCDTransaction];
        let bitmask = preferences_to_bitmask(preferences.clone().into_iter());

        let decoded_preferences = bitmask_to_preferences(bitmask);
        let expected_preferences_set = HashSet::from_iter(preferences.into_iter());
        let decoded_preferences_set = decoded_preferences;

        assert_eq!(decoded_preferences_set, expected_preferences_set);
    }

    #[test]
    fn test_single_preference_to_bitmask_and_back() {
        let preferences = vec![Preference::CIS2Transaction];
        let bitmask = preferences_to_bitmask(preferences.clone().into_iter());
        assert_eq!(bitmask, PREFERENCE_MAP[&Preference::CIS2Transaction]);

        let decoded_preferences = bitmask_to_preferences(bitmask);
        assert_eq!(
            decoded_preferences,
            HashSet::from_iter(preferences.into_iter())
        );

        let preferences2 = vec![Preference::CCDTransaction];
        let bitmask2 = preferences_to_bitmask(preferences2.clone().into_iter());
        assert_eq!(bitmask2, PREFERENCE_MAP[&Preference::CCDTransaction]);

        let decoded_preferences2 = bitmask_to_preferences(bitmask2);
        assert_eq!(
            decoded_preferences2,
            HashSet::from_iter(preferences2.into_iter())
        );
    }

    #[test]
    fn test_no_preference() {
        let preferences = vec![];
        let bitmask = preferences_to_bitmask(preferences.into_iter());
        assert_eq!(bitmask, 0); // No preferences set

        let decoded_preferences = bitmask_to_preferences(bitmask);
        assert!(
            decoded_preferences.is_empty(),
            "No preferences should be decoded from a bitmask of 0."
        );
    }
}
