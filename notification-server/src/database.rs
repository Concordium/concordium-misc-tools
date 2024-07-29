use crate::models::Preference;
use anyhow::anyhow;
use lazy_static::lazy_static;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use tokio_postgres::{Client, NoTls};

#[derive(Clone)]
pub struct PreparedStatements {
    get_devices_from_account: tokio_postgres::Statement,
    upsert_device:            tokio_postgres::Statement,
    client:                   Arc<Mutex<Client>>,
}

impl PreparedStatements {
    async fn new(client: Client) -> anyhow::Result<Self> {
        let client_mutex = Arc::new(Mutex::new(client));
        let (get_devices_from_account, upsert_device) = {
            let client_guard = client_mutex.lock().await; // MutexGuard is scoped
            let get_devices_from_account = client_guard
                .prepare(
                    "SELECT device_id FROM account_device_mapping WHERE address = $1 LIMIT 1000",
                )
                .await
                .map_err(|e| anyhow!("Failed to create account device mapping: {}", e))?;
            let upsert_device = client_guard
                .prepare(
                    "INSERT INTO account_device_mapping (address, device_id, preferences) VALUES \
                     ($1, $2, $3) ON CONFLICT (address, device_id) DO UPDATE SET preferences = \
                     EXCLUDED.preferences;",
                )
                .await
                .map_err(|e| anyhow!("Failed to create account device mapping: {}", e))?;

            (get_devices_from_account, upsert_device) // Return prepared
                                                      // statements
        };
        Ok(PreparedStatements {
            get_devices_from_account,
            upsert_device,
            client: client_mutex,
        })
    }

    pub async fn get_devices_from_account(&self, address: &[u8]) -> anyhow::Result<Vec<String>> {
        let client = self.client.lock().await;
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
        let mut client = self.client.lock().await;
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

#[derive(Clone)]
pub struct DatabaseConnection {
    pub prepared: PreparedStatements,
}

impl DatabaseConnection {
    pub async fn create(conn_string: tokio_postgres::config::Config) -> anyhow::Result<Self> {
        let (client, connection) = conn_string.connect(NoTls).await?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                log::error!("Connection error: {}", e);
            }
        });

        let prepared = PreparedStatements::new(client).await?;
        Ok(DatabaseConnection { prepared })
    }
}

lazy_static! {
    static ref PREFERENCE_MAP: HashMap<Preference, i32> =
        vec![(Preference::CIS2, 1), (Preference::CCDTransaction, 2),]
            .into_iter()
            .collect();
}

pub fn preferences_to_bitmask(preferences: &[Preference]) -> i32 {
    preferences
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
    use std::collections::HashSet;

    #[test]
    fn test_preferences_to_bitmask_and_back() {
        let preferences = vec![Preference::CIS2, Preference::CCDTransaction];
        let bitmask = preferences_to_bitmask(&preferences);

        let decoded_preferences = bitmask_to_preferences(bitmask);
        let expected_preferences_set: HashSet<_> = preferences.into_iter().collect();
        let decoded_preferences_set: HashSet<_> = decoded_preferences.into_iter().collect();

        assert_eq!(decoded_preferences_set, expected_preferences_set);
    }

    #[test]
    fn test_single_preference_to_bitmask_and_back() {
        let preferences = vec![Preference::CIS2];
        let bitmask = preferences_to_bitmask(&preferences);
        assert_eq!(bitmask, 1); // Only CIS2

        let decoded_preferences = bitmask_to_preferences(bitmask);
        assert_eq!(decoded_preferences, preferences);

        let preferences2 = vec![Preference::CCDTransaction];
        let bitmask2 = preferences_to_bitmask(&preferences2);
        assert_eq!(bitmask2, 2); // Only CCDTransaction

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