use crate::models::Preference;
use anyhow::{anyhow, Context};
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use lazy_static::lazy_static;
use std::collections::{HashMap, HashSet};
use tokio_postgres::NoTls;

#[derive(Clone, Debug)]
pub struct PreparedStatements {
    get_devices_from_account: tokio_postgres::Statement,
    upsert_device:            tokio_postgres::Statement,
    pool:                     Pool,
}

impl PreparedStatements {
    async fn new(pool: Pool) -> anyhow::Result<Self> {
        let mut client = pool.get().await.context("Failed to get client")?;
        let transaction = client
            .transaction()
            .await
            .context("Failed to start a transaction")?;
        let get_devices_from_account = transaction
            .prepare("SELECT device_id FROM account_device_mapping WHERE address = $1 LIMIT 1000")
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
        transaction
            .commit()
            .await
            .context("Failed to commit transaction")?;
        Ok(PreparedStatements {
            get_devices_from_account,
            upsert_device,
            pool,
        })
    }

    pub async fn get_devices_from_account(&self, address: &[u8]) -> anyhow::Result<Vec<String>> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|e| anyhow!("Failed to get client: {}", e))?;
        let params: &[&(dyn tokio_postgres::types::ToSql + Sync)] = &[&address];
        let rows = client.query(&self.get_devices_from_account, params).await?;
        let devices: Vec<String> = rows
            .iter()
            .map(|row| row.try_get::<_, String>(0))
            .collect::<Result<Vec<String>, _>>()?;
        Ok(devices)
    }

    pub async fn upsert_subscription(
        &self,
        address: Vec<Vec<u8>>,
        preferences: Vec<Preference>,
        device_id: &str,
    ) -> anyhow::Result<()> {
        let mut client = self
            .pool
            .get()
            .await
            .map_err(|e| anyhow!("Failed to get client: {}", e))?;
        let preferences_mask = preferences_to_bitmask(&preferences);
        let transaction = client.transaction().await?;
        for account in address {
            let params: &[&(dyn tokio_postgres::types::ToSql + Sync)] =
                &[&account, &device_id, &preferences_mask];
            if let Err(e) = transaction.execute(&self.upsert_device, params).await {
                let _ = transaction.rollback().await;
                return Err(e.into());
            }
        }
        transaction.commit().await.map_err(Into::into)
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

pub fn preferences_to_bitmask(preferences: &[Preference]) -> i32 {
    let unique_preferences: HashSet<Preference> = preferences.iter().copied().collect();
    unique_preferences
        .iter()
        .fold(0, |acc, &pref| acc | PREFERENCE_MAP[&pref])
}

pub fn bitmask_to_preferences(bitmask: i32) -> Vec<Preference> {
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
    use super::*;
    use enum_iterator::all;
    use std::collections::HashSet;

    #[test]
    fn test_preference_map_coverage_and_uniqueness() {
        let expected_variants = all::<Preference>().collect::<Vec<_>>();

        // Check for coverage
        for variant in &expected_variants {
            assert!(
                PREFERENCE_MAP.contains_key(variant),
                "PREFERENCE_MAP is missing the variant {:?}",
                variant
            );
        }

        // Check for uniqueness of indices
        let mut indices = vec![];
        for &index in PREFERENCE_MAP.values() {
            assert!(
                !indices.contains(&index),
                "Duplicate index found: {}",
                index
            );
            indices.push(index);
        }

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
        let bitmask = preferences_to_bitmask(&preferences);

        let decoded_preferences = bitmask_to_preferences(bitmask);
        let expected_preferences_set: HashSet<_> = preferences.into_iter().collect();
        let decoded_preferences_set: HashSet<_> = decoded_preferences.into_iter().collect();

        assert_eq!(decoded_preferences_set, expected_preferences_set);
    }

    #[test]
    fn test_single_preference_to_bitmask_and_back() {
        let preferences = vec![Preference::CIS2Transaction];
        let bitmask = preferences_to_bitmask(&preferences);
        assert_eq!(bitmask, 1);

        let decoded_preferences = bitmask_to_preferences(bitmask);
        assert_eq!(decoded_preferences, preferences);

        let preferences2 = vec![Preference::CCDTransaction];
        let bitmask2 = preferences_to_bitmask(&preferences2);
        assert_eq!(bitmask2, 2);

        let decoded_preferences2 = bitmask_to_preferences(bitmask2);
        assert_eq!(decoded_preferences2, preferences2);
    }

    #[test]
    fn test_no_preference() {
        let preferences = vec![];
        let bitmask = preferences_to_bitmask(&preferences);
        assert_eq!(bitmask, 0); // No preferences

        let decoded_preferences = bitmask_to_preferences(bitmask);
        assert_eq!(decoded_preferences, preferences);
    }
}
