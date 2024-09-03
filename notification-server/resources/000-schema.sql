-- accounts created.
CREATE TABLE IF NOT EXISTS account_device_mapping (
  id SERIAL8 PRIMARY KEY,
  address BYTEA NOT NULL,
  device_id VARCHAR NOT NULL,
  preferences INTEGER NOT NULL,
  UNIQUE (address, device_id)
);

CREATE TABLE IF NOT EXISTS blocks (
  id SERIAL8 PRIMARY KEY,
  hash BYTEA NOT NULL UNIQUE,
  height INT8 NOT NULL UNIQUE
);
