use tokio_postgres::{Client, NoTls};

pub struct PreparedStatements {
    get_devices_from_account: tokio_postgres::Statement,
    upsert_device: tokio_postgres::Statement,
    client:                   Client,
}

impl PreparedStatements {
    async fn new(client: Client) -> Result<Self, tokio_postgres::Error> {
        let get_devices_from_account = client
            .prepare("SELECT device_id FROM account_device_mapping WHERE address = $1")
            .await?;
        let upsert_device = client
            .prepare("INSERT INTO account_device_mapping (address, device_id) VALUES ($1, $2) ON CONFLICT (address, device_id) DO NOTHING")
            .await?;
        Ok(PreparedStatements {
            get_devices_from_account,
            upsert_device,
            client,
        })
    }

    pub async fn get_devices_from_account(&self, address: &[u8]) -> anyhow::Result<Vec<String>> {
        let params: &[&(dyn tokio_postgres::types::ToSql + Sync)] = &[&address];
        let rows = &self
            .client
            .query(&self.get_devices_from_account, params)
            .await?;
        let devices: Vec<String> = rows
            .iter()
            .map(|row| row.try_get::<_, String>(0))
            .collect::<Result<Vec<String>, _>>()?;
        Ok(devices)
    }

    pub async fn upsert_device(&self, address: &[u8], device_id: &str) -> anyhow::Result<()> {
        let params: &[&(dyn tokio_postgres::types::ToSql + Sync)] = &[&address, device_id];
        &self.client.execute(&self.upsert_device, params).await?;
        Ok(())
    }
}

pub struct DatabaseConnection {
    pub prepared:      PreparedStatements,
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
        Ok(DatabaseConnection {
            prepared,
        })
    }
}
