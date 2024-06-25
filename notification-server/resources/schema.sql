-- accounts created.
CREATE TABLE IF NOT EXISTS account_device_mapping (
  id SERIAL8 PRIMARY KEY,
  address BYTEA NOT NULL,
  device_id VARCHAR NOT NULL,
  UNIQUE (address, device_id)
);
