-- Floors a (bigint) timestamp in seconds into 24h slots in seconds, e.g. 15/02/2022:14:00:00 -> 15/02/2022:00:00:00. This grouping matches how Grafana groups seconds into days.
CREATE OR REPLACE FUNCTION date_seconds(t bigint)
  RETURNS bigint
  LANGUAGE plpgsql
AS $$
DECLARE
  date bigint;
BEGIN
  SELECT floor((t)/86400)*86400::bigint
  INTO date;

  RETURN date;
END;
$$;

-- Returns whether a given transaction type is a CCD transfer.
CREATE OR REPLACE FUNCTION is_ccd_transfer(type int2)
  RETURNS boolean
  LANGUAGE sql
  IMMUTABLE
  -- The types correspond to 3: Transfer, 10: EncryptedAmountTransfer, 13: TransferWithMemo, 16: TransferWithSchedule, 17: EncryptedAmountTransferWithMemo, 18: TransferWithScheduleAndMemo
  RETURN type in (3, 10, 13, 16, 17, 18);

-- All blocks. Mostly act as a time reference for other entities.
CREATE TABLE IF NOT EXISTS blocks (
  id SERIAL8 PRIMARY KEY,
  hash BYTEA NOT NULL UNIQUE,
  timestamp INT8 NOT NULL,
  height INT8 NOT NULL
);
-- Create index on block timestamp to improve performance when querying for blocks within a timerange.
CREATE INDEX IF NOT EXISTS blocks_timestamp ON blocks (timestamp);

-- All payday blocks (only exists for protocol version 4 and onwards).
CREATE TABLE IF NOT EXISTS paydays (
  block INT8 PRIMARY KEY REFERENCES blocks(id) ON DELETE RESTRICT ON UPDATE RESTRICT,
  total_stake INT8 NOT NULL,
  num_bakers INT8 -- Only supported for protocol version 6 and onwards.
);

-- All accounts created.
CREATE TABLE IF NOT EXISTS accounts (
  id SERIAL8 PRIMARY KEY,
  address BYTEA NOT NULL UNIQUE,
  block INT8 NOT NULL REFERENCES blocks(id) ON DELETE RESTRICT ON UPDATE RESTRICT, -- To support time series output.
  is_initial BOOLEAN NOT NULL
);

-- All smart contract modules deployed
CREATE TABLE IF NOT EXISTS modules (
  id SERIAL8 PRIMARY KEY,
  ref BYTEA NOT NULL UNIQUE,
  block INT8 NOT NULL REFERENCES blocks(id) ON DELETE RESTRICT ON UPDATE RESTRICT -- To support time series output.
);

-- All smart contract instances created
CREATE TABLE IF NOT EXISTS contracts (
  id SERIAL8 PRIMARY KEY,
  index INT8 NOT NULL,
  subindex INT8 NOT NULL,
  module INT8 NOT NULL REFERENCES modules(id) ON DELETE RESTRICT ON UPDATE RESTRICT,
  block INT8 NOT NULL REFERENCES blocks(id) ON DELETE RESTRICT ON UPDATE RESTRICT, -- To support time series output.
  UNIQUE (index, subindex)
);

-- All account transactions
CREATE TABLE IF NOT EXISTS transactions (
  id INT8 PRIMARY KEY,
  hash BYTEA NOT NULL UNIQUE,
  block INT8 NOT NULL REFERENCES blocks(id) ON DELETE RESTRICT ON UPDATE RESTRICT, -- To support time series output.
  cost INT8 NOT NULL,
  is_success BOOLEAN NOT NULL,
  type INT2 -- NULL means the transaction was rejected
);
-- Create index on transactions reference to block to improve performance when querying for transactions linked to specific blocks.
CREATE INDEX IF NOT EXISTS transactions_block ON transactions (block);

-- Keeps track of relations between accounts and transactions to support account activeness.
CREATE TABLE IF NOT EXISTS accounts_transactions (
  account INT8 NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT ON UPDATE RESTRICT,
  transaction INT8 NOT NULL REFERENCES transactions(id) ON DELETE RESTRICT ON UPDATE RESTRICT
);
-- Create index on account-transaction relations references to transactions to improve performance when querying accounts related to specific transactions.
CREATE INDEX IF NOT EXISTS at_transaction ON accounts_transactions (transaction);

-- Keeps track of relations between contracts and transactions to support contract activeness.
CREATE TABLE IF NOT EXISTS contracts_transactions (
  contract INT8 NOT NULL REFERENCES contracts(id) ON DELETE RESTRICT ON UPDATE RESTRICT,
  transaction INT8 NOT NULL REFERENCES transactions(id) ON DELETE RESTRICT ON UPDATE RESTRICT
);
-- Create index on contract-transaction relations references to transactions to improve performance when querying contracts related to specific transactions.
CREATE INDEX IF NOT EXISTS ct_transaction ON contracts_transactions (transaction);

-- Create table keeping track of accounts and the dates they have been part of a transations.
CREATE TABLE IF NOT EXISTS account_activeness (
  account INT8 NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT ON UPDATE RESTRICT, -- dependant on inserts into `accounts_transactions`.
  time INT8 NOT NULL, -- Date represented in seconds, rounded value of the `timestamp` column on `blocks` corresponding to the block the transaction was a part of.
  UNIQUE (account, time)
);
-- To support binary search instead of sequential scan of active accounts.
CREATE INDEX IF NOT EXISTS account_activeness_time ON account_activeness (time);

-- Create table keeping track of contracts and the dates they have been part of a transations.
CREATE TABLE IF NOT EXISTS contract_activeness (
  contract INT8 NOT NULL REFERENCES contracts(id) ON DELETE RESTRICT ON UPDATE RESTRICT, -- dependant on inserts into `contracts_transactions`.
  time INT8 NOT NULL, -- Date represented in seconds, rounded value of the `timestamp` column on `blocks` corresponding to the block the transaction was a part of.
  UNIQUE (contract, time)
);
-- To support binary search instead of sequential scan of active contracts.
CREATE INDEX IF NOT EXISTS contract_activeness_time ON contract_activeness (time);
