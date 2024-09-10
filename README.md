## Polkadot staking miner monitor

This is a simple tool that monitors each election in polkadot, kusama and westend 
then stores the following data related in a postgres database:
- submissions: The list of all submissions in each election, this is regarded 
  as successful if the solution extrinsic is accepted by the chain. The solution may
  be rejected at the end of the election when it's fully verified. You need check `slashed`
  together with this to know whether a solution was truly valid.
- winners: Get the winners of each round
- slashed: Get the slashed accounts

The tool is based on the subxt library and is written in Rust.

## Web APIs
- `GET /docs/` - swagger UI
- `GET /docs/openapi.json` - OpenAPI JSON schema
- `GET /docs/openapi.yaml` - OpenAPI YAML schema
- `GET /submissions` - Get all submissions from the database in JSON format.
- `GET /submissions/{n}` - Get the `n` most recent submissions from the database in JSON format, n is a number.
- `GET /winners` - Dump all winners from the database in JSON format.
- `GET /winners/{n}` - Get the `n` most recent winners from the database in JSON format, n is a number.
- `GET /unsigned-winners` - Get all winners that was submitted by a validator (this is fail-safe mechanism when no staking miner is available).
- `GET /unsigned-winners/{n}` - Get the `n` most recent unsigned winners from the database in JSON format, n is a number.
- `GET /slashed` - Get all slashed solutions from the database in JSON format.
- `GET /slashed/{n}` - Get the `n` most recent slashed solutions from the database in JSON format, n is a number.

## Roadmap

1. Add functionality to start syncing from a specific block instead of the latest. To get the full history of the chain.
2. More sophisticated API to get more detailed information about the submissions and the winners.
3. Support older metadata versions.

### Limitations

This tool is based on the subxt and this means that it is limited to blocks with metadata v14
and why full history is not supported.

### Usage

```bash
$ cargo run --release -- --polkadot wss://rpc.polkadot.io --postgres "postgresql://user:pwd@localhost/polkadot"
```

Open another terminal and run the following commands to use the API:

#### Get all submissions

```bash
$ curl "http://localhost:9999/submissions"
[
    {"who":"0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d","round":79,"block":1564,"score":{"minimal_stake":100000000000000,"sum_stake":100000000000000,"sum_stake_squared":10000000000000000000000000000},"success":true},
    {"who":"0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d","round":80,"block":1584,"score":{"minimal_stake":100000000000000,"sum_stake":100000000000000,"sum_stake_squared":10000000000000000000000000000},"success":true},
    {"who":"0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d","round":81,"block":1604,"score":{"minimal_stake":340282366920938463463374607431768211455,"sum_stake":340282366920938463463374607431768211455,"sum_stake_squared":340282366920938463463374607431768211455},"success":true},
    {"who":"unsigned","round":81,"block":1612,"score":{"minimal_stake":100000000000000,"sum_stake":100000000000000,"sum_stake_squared":10000000000000000000000000000},"success":true}
]
```

#### Get the most recent submission
```bash
$ curl "http://localhost:9999/submissions/1"
[{"who":"unsigned","round":82,"block":1632,"score":{"minimal_stake":100000000000000,"sum_stake":100000000000000,"sum_stake_squared":10000000000000000000000000000},"success":true}]
```

#### Get all winners

```bash
$ curl "http://localhost:9999/winners"
[
    {"who":"0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d","round":80,"block":1581,"score":{"minimal_stake":100000000000000,"sum_stake":100000000000000,"sum_stake_squared":10000000000000000000000000000}},
    {"who":"0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d","round":81,"block":1601,"score":{"minimal_stake":100000000000000,"sum_stake":100000000000000,"sum_stake_squared":10000000000000000000000000000}},
    {"who":"unsigned","round":82,"block":1621,"score":{"minimal_stake":100000000000000,"sum_stake":100000000000000,"sum_stake_squared":10000000000000000000000000000}}
]
```

#### Get the most recent winner

```bash
$ curl "http://localhost:9999/winners/1"
[
    {"who":"unsigned","round":82,"block":1621,"score":{"minimal_stake":100000000000000,"sum_stake":100000000000000,"sum_stake_squared":10000000000000000000000000000}}
]
```

#### Get all unsigned winners

```bash
$ curl "http://localhost:9999/unsigned-winners"
[
    {"who":"unsigned","round":82,"block":1621,"score":{"minimal_stake":100000000000000,"sum_stake":100000000000000,"sum_stake_squared":10000000000000000000000000000}},
    {"who":"unsigned","round":83,"block":1641,"score":{"minimal_stake":100000000000000,"sum_stake":100000000000000,"sum_stake_squared":10000000000000000000000000000}}
]
```

#### Get the most recent unsigned winner

```bash
$ curl "http://localhost:9999/unsigned-winners/1"
[
    {"who":"unsigned","round":83,"block":1641,"score":{"minimal_stake":100000000000000,"sum_stake":100000000000000,"sum_stake_squared":10000000000000000000000000000}}
]
```

#### Get all slashed solutions

```bash
$ curl "http://localhost:9999/slashed"
[
    {"who":"0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d","round":81,"block":1611,"amount":"2000034179670"},
    {"who":"0x8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48","round":85,"block":1691,"amount":"2000034179670"}]
```

#### Get the most recent slashed

```bash
$ curl "http://localhost:9999/slashed/1"
[
    {"who":"0x8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48","round":85,"block":1691,"amount":"2000034179670"}
]
```

### Database migrations

This tool has a simple database with three tables: `submissions`, `election_winners` and `slashed` which is located in the `migrations` folder.
To add a new migration, just create a new file with the following format: `V{version}__{description}.sql` and it will be automatically applied when the tool is started.