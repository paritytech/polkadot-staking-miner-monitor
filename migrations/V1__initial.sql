CREATE TABLE IF NOT EXISTS submissions (
    id SERIAL PRIMARY KEY,
    address TEXT,
    round OID,
    block OID,
    score JSONB,
    success BOOLEAN
);

CREATE TABLE IF NOT EXISTS elections
(
    id SERIAL PRIMARY KEY,
    result TEXT,
    address JSONB,
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