-- All blocks. Mostly act as a time reference for other entities.
CREATE TABLE IF NOT EXISTS blocks (
  id SERIAL8 PRIMARY KEY,
  hash BYTEA NOT NULL UNIQUE,
  timestamp INT8 NOT NULL,
  height INT8 NOT NULL,
  total_stake INT8 -- NULL means the block is NOT a payday block.
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
  id SERIAL8 PRIMARY KEY,
  hash BYTEA NOT NULL UNIQUE,
  block INT8 NOT NULL REFERENCES blocks(id) ON DELETE RESTRICT ON UPDATE RESTRICT, -- To support time series output.
  cost INT8 NOT NULL,
  is_success BOOLEAN NOT NULL,
  type INT2 -- NULL means the transaction was rejected
);

-- Create index on transaction type to improve performance when querying for transactions of specific types.
CREATE INDEX IF NOT EXISTS transactions_type ON transactions USING HASH (type);

-- Create index on transactions reference to block to improve performance when querying for transactions linked to specific blocks. This is heavily used for account/contract activeness.
CREATE INDEX IF NOT EXISTS transactions_block ON transactions USING HASH (block);

-- Keeps track of relations between accounts and transactions to support account activeness.
CREATE TABLE IF NOT EXISTS accounts_transactions (
  account INT8 NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT ON UPDATE RESTRICT,
  transaction INT8 NOT NULL REFERENCES transactions(id) ON DELETE RESTRICT ON UPDATE RESTRICT
);

-- Keeps track of relations between contracts and transactions to support contract activeness.
CREATE TABLE IF NOT EXISTS contracts_transactions (
  contract INT8 NOT NULL REFERENCES contracts(id) ON DELETE RESTRICT ON UPDATE RESTRICT,
  transaction INT8 NOT NULL REFERENCES transactions(id) ON DELETE RESTRICT ON UPDATE RESTRICT
);
