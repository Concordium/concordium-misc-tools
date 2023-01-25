CREATE TABLE IF NOT EXISTS blocks (
  id SERIAL8 PRIMARY KEY,
  hash BYTEA NOT NULL UNIQUE,
  timestamp INT8 NOT NULL,
  height INT8 NOT NULL,
  total_stake INT8 -- NULL means the block is NOT a payday block.
);

CREATE TABLE IF NOT EXISTS accounts (
  id SERIAL8 PRIMARY KEY,
  address BYTEA NOT NULL UNIQUE,
  block INT8 NOT NULL REFERENCES blocks(id) ON DELETE RESTRICT ON UPDATE RESTRICT, -- To support time series output.
  is_initial BOOLEAN NOT NULL
);

CREATE TABLE IF NOT EXISTS modules (
  id SERIAL8 PRIMARY KEY,
  ref BYTEA NOT NULL UNIQUE,
  block INT8 NOT NULL REFERENCES blocks(id) ON DELETE RESTRICT ON UPDATE RESTRICT -- To support time series output.
);

CREATE TABLE IF NOT EXISTS contracts (
  id SERIAL8 PRIMARY KEY,
  index INT8 NOT NULL,
  subindex INT8 NOT NULL,
  module BYTEA NOT NULL,
  block INT8 NOT NULL REFERENCES blocks(id) ON DELETE RESTRICT ON UPDATE RESTRICT, -- To support time series output.
  UNIQUE (index, subindex)
);

CREATE TABLE IF NOT EXISTS transactions (
  id SERIAL8 PRIMARY KEY,
  hash BYTEA NOT NULL UNIQUE,
  block INT8 NOT NULL REFERENCES blocks(id) ON DELETE RESTRICT ON UPDATE RESTRICT, -- To support time series output.
  type INT2 -- NULL means the transaction was rejected
);

-- Keeps track of relations between accounts and transactions to support account activeness.
CREATE TABLE IF NOT EXISTS accounts_transactions (
  account INT8 NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT ON UPDATE RESTRICT,
  transaction INT8 NOT NULL REFERENCES transactions(id) ON DELETE RESTRICT ON UPDATE RESTRICT,
  CONSTRAINT accounts_transactions_unique UNIQUE (account, transaction) -- Ensures only unique rows can be inserted
);

-- Keeps track of relations between contracts and transactions to support contract activeness.
CREATE TABLE IF NOT EXISTS contracts_transactions (
  contract INT8 NOT NULL REFERENCES contracts(id) ON DELETE RESTRICT ON UPDATE RESTRICT,
  transaction INT8 NOT NULL REFERENCES transactions(id) ON DELETE RESTRICT ON UPDATE RESTRICT,
  CONSTRAINT contracts_transactions_unique UNIQUE (contract, transaction) -- Ensures only unique rows can be inserted
);

-- Create index on transaction type to improve performance when querying for transactions of specific types.
CREATE INDEX IF NOT EXISTS transactions_type ON transactions USING HASH (type);
