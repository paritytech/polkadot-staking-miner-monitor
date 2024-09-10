CREATE TABLE IF NOT EXISTS submissions (
    id SERIAL PRIMARY KEY,
    address TEXT,
    round OID,
    block OID,
    score JSONB,
    success BOOLEAN
);

CREATE TABLE IF NOT EXISTS election_winners 
(
    id SERIAL PRIMARY KEY,
    address TEXT,
    round OID,
    block OID,
    score JSONB
);

CREATE TABLE IF NOT EXISTS slashed (
    id SERIAL PRIMARY KEY,
    address TEXT,
    amount TEXT,
    round OID,
    block OID
);