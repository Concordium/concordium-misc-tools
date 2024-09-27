use crate::models::device::{Device, Preference};
use concordium_rust_sdk::{
    base::hashes::BlockHash, common::types::AccountAddress, types::AbsoluteBlockHeight,
};
use deadpool_postgres::{GenericClient, Manager, ManagerConfig, Pool, PoolError, RecyclingMethod};
use lazy_static::lazy_static;
use log::error;
use std::{
    collections::{HashMap, HashSet},
    vec::IntoIter,
};
use thiserror::Error;
use tokio_postgres::{error::SqlState, types::ToSql, NoTls};

#[derive(Debug, Error)]
pub enum Error {
    #[error("Unrecoverable connection issue: {0}")]
    DatabaseConnection(#[from] tokio_postgres::Error),
    #[error("Unrecoverable pool issue: {0}")]
    PoolError(#[from] PoolError),
    #[error(
        "Failed inserting block with hash {0} because one with height of {1} has already been \
         inserted"
    )]
    ConstraintViolation(BlockHash, AbsoluteBlockHeight),
}

#[derive(Clone, Debug)]
pub struct DatabaseConnection(Pool);

impl DatabaseConnection {
    pub async fn create(config: tokio_postgres::config::Config) -> anyhow::Result<Self> {
        let mgr_config = ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        };
        let mgr = Manager::from_config(config, NoTls, mgr_config);
        let pool = Pool::builder(mgr)
            .max_size(16)
            .build()
            .expect("Failed to create pool");
        Ok(DatabaseConnection(pool))
    }

    pub async fn get_processed_block_height(&self) -> Result<Option<AbsoluteBlockHeight>, Error> {
        let client = self.0.get().await.map_err(Into::<Error>::into)?;
        let stmt = client
            .prepare_cached(
                "SELECT blocks.height FROM blocks WHERE blocks.id = (SELECT MAX(blocks.id) FROM \
                 blocks);",
            )
            .await
            .map_err(Into::<Error>::into)?;
        let row = client.query_opt(&stmt, &[]).await?;
        row.map(|row| row.try_get::<_, i64>(0).map(|raw| (raw as u64).into()))
            .transpose()
            .map_err(Into::into)
    }

    pub async fn get_devices_from_account(
        &self,
        account_address: &AccountAddress,
    ) -> Result<Vec<Device>, Error> {
        let client = self.0.get().await.map_err(Into::<Error>::into)?;
        let stmt = client
            .prepare_cached(
                "SELECT device_id, preferences FROM account_device_mapping WHERE address = $1 \
                 LIMIT 1000",
            )
            .await
            .map_err(Into::<Error>::into)?;
        let params: &[&(dyn ToSql + Sync)] = &[&account_address.0.as_ref()];
        let rows = client
            .query(&stmt, params)
            .await
            .map_err(Into::<Error>::into)?;
        rows.iter()
            .map(|row| {
                let device_token = row.try_get::<_, String>(0)?;
                let preferences = bitmask_to_preferences(row.try_get::<_, i32>(1)?);
                Ok(Device::new(preferences, device_token))
            })
            .collect::<Result<Vec<Device>, _>>()
    }

    pub async fn remove_subscription(&self, device_token: &str) -> Result<u64, Error> {
        let client = self.0.get().await.map_err(Into::<Error>::into)?;
        let params: &[&(dyn ToSql + Sync)] = &[&device_token];
        let stmt = client
            .prepare_cached("DELETE FROM account_device_mapping WHERE device_id = $1;")
            .await
            .map_err(Into::<Error>::into)?;
        client
            .execute(&stmt, params)
            .await
            .map_err(Error::DatabaseConnection)
    }

    pub async fn upsert_subscription(
        &self,
        account_address: Vec<AccountAddress>,
        preferences: Vec<Preference>,
        device_token: &str,
    ) -> Result<(), Error> {
        let mut client = self.0.get().await.map_err(Into::<Error>::into)?;
        let stmt = client
            .prepare_cached(
                "INSERT INTO account_device_mapping (address, device_id, preferences) VALUES ($1, \
                 $2, $3) ON CONFLICT (address, device_id) DO UPDATE SET preferences = \
                 EXCLUDED.preferences;",
            )
            .await
            .map_err(Into::<Error>::into)?;
        let preferences_mask = preferences_to_bitmask(preferences.into_iter());
        let transaction = client.transaction().await?;
        for account in account_address {
            let params: &[&(dyn ToSql + Sync)] =
                &[&account.0.as_ref(), &device_token, &preferences_mask];
            if let Err(e) = transaction.execute(&stmt, params).await {
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
    ) -> Result<(), Error> {
        let client = self.0.get().await.map_err(Into::<Error>::into)?;
        let stmt = client
            .prepare_cached("INSERT INTO blocks (hash, height) VALUES ($1, $2);")
            .await
            .map_err(Into::<Error>::into)?;
        let params: &[&(dyn ToSql + Sync); 2] = &[&hash.as_ref(), &(height.height as i64)];
        client.execute(&stmt, params).await.map_or_else(
            |err| {
                if let Some(db_err) = err.as_db_error() {
                    if db_err.code() == &SqlState::UNIQUE_VIOLATION {
                        return Err(Error::ConstraintViolation(*hash, *height));
                    }
                };
                Err(Error::DatabaseConnection(err))
            },
            |_| Ok(()),
        )
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
    use super::*;
    use crate::models::device::Preference::{CCDTransaction, CIS2Transaction};
    use dotenv::dotenv;
    use enum_iterator::all;
    use serial_test::serial;
    use std::{collections::HashSet, env, fs, path::Path, str::FromStr};
    use tokio_postgres::Client;

    #[test]
    fn test_preference_map_coverage_and_uniqueness() {
        let expected_variants = all::<Preference>().collect::<HashSet<_>>();
        for variant in &expected_variants {
            assert!(
                PREFERENCE_MAP.contains_key(variant),
                "PREFERENCE_MAP is missing the variant {:?}",
                variant
            );
        }

        let indices = PREFERENCE_MAP.values().cloned().collect::<HashSet<_>>();
        assert_eq!(
            indices.len(),
            PREFERENCE_MAP.len(),
            "Indices in PREFERENCE_MAP are not unique."
        );

        assert_eq!(
            PREFERENCE_MAP.len(),
            expected_variants.len(),
            "PREFERENCE_MAP does not match the number of variants in Preference enum"
        );
    }

    #[test]
    fn test_preferences_to_bitmask_and_back() {
        let preferences = vec![CIS2Transaction, CCDTransaction];
        let bitmask = preferences_to_bitmask(preferences.clone().into_iter());

        let decoded_preferences = bitmask_to_preferences(bitmask);
        let expected_preferences_set = HashSet::from_iter(preferences);
        let decoded_preferences_set = decoded_preferences;

        assert_eq!(decoded_preferences_set, expected_preferences_set);
    }

    #[test]
    fn test_single_preference_to_bitmask_and_back() {
        let preferences = vec![CIS2Transaction];
        let bitmask = preferences_to_bitmask(preferences.clone().into_iter());
        assert_eq!(bitmask, PREFERENCE_MAP[&CIS2Transaction]);

        let decoded_preferences = bitmask_to_preferences(bitmask);
        assert_eq!(
            decoded_preferences,
            HashSet::from_iter(preferences.into_iter())
        );

        let preferences2 = vec![CCDTransaction];
        let bitmask2 = preferences_to_bitmask(preferences2.clone().into_iter());
        assert_eq!(bitmask2, PREFERENCE_MAP[&CCDTransaction]);

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

    async fn drop_all_tables(client: &Client) -> Result<(), tokio_postgres::Error> {
        let rows = client
            .query(
                "SELECT tablename FROM pg_tables WHERE schemaname = current_schema()",
                &[],
            )
            .await?;

        for row in rows {
            let table_name: &str = row.get(0);
            client
                .batch_execute(&format!("DROP TABLE IF EXISTS {} CASCADE;", table_name))
                .await?;
        }
        Ok(())
    }

    async fn create_sql(client: &Client) -> Result<(), tokio_postgres::Error> {
        let sql_directory = Path::new("resources");
        for entry in fs::read_dir(sql_directory).expect("Failed to read SQL directory") {
            let entry = entry.expect("Failed to read directory entry");
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("sql") {
                let sql = fs::read_to_string(&path).expect("Unable to read SQL file");
                client.batch_execute(&sql).await?;
            }
        }
        Ok(())
    }

    async fn setup_database() -> anyhow::Result<DatabaseConnection> {
        dotenv().ok();
        let config = env::var("NOTIFICATION_SERVER_DB_CONNECTION")
            .unwrap()
            .parse()
            .unwrap();
        let db_connection = DatabaseConnection::create(config).await?;

        let client = db_connection.0.get().await?;
        drop_all_tables(&client).await?;
        create_sql(&client).await?;

        Ok(db_connection)
    }

    #[tokio::test]
    #[serial]
    async fn test_get_devices_from_account() {
        let db_connection = setup_database().await.unwrap();
        let account_address =
            AccountAddress::from_str("4FmiTW2L2AccyR9VjzsnpWFSAcohXWf7Vf797i36y526mqiEcp").unwrap();
        let device = "device-1";
        db_connection
            .upsert_subscription(
                vec![account_address],
                vec![Preference::CIS2Transaction],
                device,
            )
            .await
            .unwrap();
        let devices = db_connection
            .get_devices_from_account(&account_address)
            .await
            .unwrap();

        let expected_devices = vec![Device::new(
            HashSet::from_iter(vec![Preference::CIS2Transaction].into_iter()),
            device.to_string(),
        )];
        assert_eq!(devices, expected_devices);
    }

    #[tokio::test]
    #[serial]
    async fn test_multiple_upsert_subscriptions() {
        let db_connection = setup_database().await.unwrap();
        let account_address =
            AccountAddress::from_str("4FmiTW2L2AccyR9VjzsnpWFSAcohXWf7Vf797i36y526mqiEcp").unwrap();
        let device = "device-1";
        db_connection
            .upsert_subscription(vec![account_address], vec![CIS2Transaction], device)
            .await
            .unwrap();
        db_connection
            .upsert_subscription(
                vec![account_address],
                vec![CIS2Transaction, CCDTransaction],
                device,
            )
            .await
            .unwrap();
        let devices = db_connection
            .get_devices_from_account(&account_address)
            .await
            .unwrap();

        assert_eq!(devices, vec![Device::new(
            HashSet::from_iter(vec![CIS2Transaction, CCDTransaction].into_iter()),
            device.to_string()
        )]);

        db_connection
            .upsert_subscription(vec![account_address], vec![], device)
            .await
            .unwrap();
        let devices = db_connection
            .get_devices_from_account(&account_address)
            .await
            .unwrap();
        assert_eq!(devices, vec![Device::new(
            HashSet::from_iter(vec![].into_iter()),
            device.to_string()
        )]);
    }

    #[tokio::test]
    #[serial]
    async fn test_insert_block() {
        let db_connection = setup_database().await.unwrap();

        let hash = BlockHash::new([0; 32]); // Example block hash
        let height = AbsoluteBlockHeight::from(1);

        db_connection.insert_block(&hash, &height).await.unwrap();

        let latest_height = db_connection.get_processed_block_height().await.unwrap();
        assert_eq!(latest_height, Some(height));

        let hash = BlockHash::new([1; 32]); // Example block hash
        let height = AbsoluteBlockHeight::from(2);

        db_connection.insert_block(&hash, &height).await.unwrap();
        let latest_height = db_connection.get_processed_block_height().await.unwrap();
        assert_eq!(latest_height.unwrap().height, 2);
    }

    #[tokio::test]
    #[serial]
    async fn test_insert_block_duplicate_hash() {
        let db_connection = setup_database().await.unwrap();

        let expected_hash = [2; 32];
        let expected_height = AbsoluteBlockHeight::from(1);

        db_connection
            .insert_block(
                &BlockHash::new(expected_hash),
                &AbsoluteBlockHeight::from(2),
            )
            .await
            .unwrap();

        if db_connection
            .insert_block(&BlockHash::new(expected_hash), &expected_height)
            .await
            .is_err()
        {
            panic!("Expected ok result");
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_insert_block_duplicate_height() {
        let db_connection = setup_database().await.unwrap();

        let expected_hash = [1; 32];
        let expected_height = AbsoluteBlockHeight::from(1);

        db_connection
            .insert_block(&BlockHash::new([0; 32]), &expected_height)
            .await
            .unwrap();

        match db_connection
            .insert_block(&BlockHash::new(expected_hash), &expected_height)
            .await
        {
            Err(Error::ConstraintViolation(ref hash, ref height)) => {
                assert_eq!(expected_hash, hash.bytes);
                assert_eq!(expected_height.height, height.height);
            }
            _ => panic!("Expected ConstraintViolation error"),
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_remove_single_subscription() {
        let db_connection = setup_database().await.expect("Failed to setup database");
        let account_address =
            AccountAddress::from_str("3144VUDLmyeNppwnRY91nP7H5bJoUbaBxiVBN7YN8XFLxMZLgH").unwrap();
        let device_not_to_delete = "device-1";
        db_connection
            .upsert_subscription(
                vec![account_address],
                vec![CIS2Transaction],
                device_not_to_delete,
            )
            .await
            .unwrap();
        let account_address =
            AccountAddress::from_str("3BY1qpYqmK8P5FBcyfvrMv72bRqvrD7sdHdim9L7oaVGZ71uFq").unwrap();
        let device_to_delete = "device-2";
        db_connection
            .upsert_subscription(
                vec![account_address],
                vec![CIS2Transaction],
                device_to_delete,
            )
            .await
            .expect("Failed to upsert");
        assert_eq!(
            db_connection
                .remove_subscription(device_to_delete)
                .await
                .expect("Failed to remove subscription"),
            1
        );
        let client = &db_connection.0.get().await.unwrap();
        let stmt = client
            .prepare_cached("SELECT device_id FROM account_device_mapping WHERE device_id = $1")
            .await
            .unwrap();
        let params: &[&(dyn ToSql + Sync)] = &[&device_not_to_delete];
        let rows = client.query(&stmt, params).await.unwrap();
        assert_eq!(rows.len(), 1);
        let params: &[&(dyn ToSql + Sync)] = &[&device_to_delete];
        let rows = client.query(&stmt, params).await.unwrap();
        assert_eq!(rows.len(), 0);
    }

    #[tokio::test]
    #[serial]
    async fn test_remove_nonexistent_subscription() {
        let db_connection = setup_database().await.expect("Failed to setup database");
        let account_address =
            AccountAddress::from_str("3144VUDLmyeNppwnRY91nP7H5bJoUbaBxiVBN7YN8XFLxMZLgH").unwrap();
        let device_existing = "device-existing";
        db_connection
            .upsert_subscription(
                vec![account_address],
                vec![CIS2Transaction],
                device_existing,
            )
            .await
            .unwrap();
        let device_not_existing = "device-none-existent";
        assert_eq!(
            db_connection
                .remove_subscription(device_not_existing)
                .await
                .expect("Failed to remove subscription"),
            0
        );
    }
}
