-- Populates account_activeness "view" with data
INSERT INTO account_activeness (account, time)
  SELECT DISTINCT at.account, date_seconds(b.timestamp) as time
  FROM blocks AS b
  JOIN transactions AS t ON b.id=t.block
  JOIN accounts_transactions AS at ON t.id=at.transaction
  ORDER BY date_seconds(b.timestamp) ASC;

-- Populates contract_activeness "view" with data
INSERT INTO contract_activeness (contract, time)
  SELECT DISTINCT ct.contract, date_seconds(b.timestamp) as time
  FROM blocks AS b
  JOIN transactions AS t ON b.id=t.block
  JOIN contracts_transactions AS ct ON t.id=ct.transaction
  ORDER BY date_seconds(b.timestamp) ASC;
