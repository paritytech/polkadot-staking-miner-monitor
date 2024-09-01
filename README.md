## Staking miner monitor

This is a simple tool that monitors each election in polkadot, kusama and westend 
then stores the solutions to the election and the winners in a SQLite database.

The tool is based on the subxt library and is written in Rust.

It exposes the following web API:

- `GET /submissions` - Get all submissions from the database in JSON format.
- `GET /submissions/{n}` - Get the `n` most recent submissions from the database in JSON format, n is a number.
- `GET /winners` - Dump all winners from the database in JSON format.
- `GET /winners/{n}` - Get the `n` most recent winners from the database in JSON format, n is a number.
- `GET /unsigned-winners` - Get all winners that was submitted by a validator (this is fail-safe mechanism when no staking miner is available).
. `GET /unsigned-winners/{n}` - Get the `n` most recent unsigned winners from the database in JSON format, n is a number.

## Roadmap

1. Add functionality to start syncing from a specific block instead of the latest. To get the full history of the contract.
2. More sophisticated API to get more detailed information about the submissions and the winners.
3. Support older metadata versions.

### Limitations

This tool is based on the subxt and this means that it is limited to blocks with metadata v14
and why full history is not supported.


### Usage

```bash
$ cargo run --release -- --url wss://rpc.polkadot.io
```

Open another terminal and run the following commands to use the API:

#### Get all submissions

```bash
$ curl "http://localhost:9999/submissions"
[
    {"address":"0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d","round":74,"block":1451,"score":{"minimal_stake":100000000000000,"sum_stake":100000000000000,"sum_stake_squared":10000000000000000000000000000}},{"address":"0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d","round":75,"block":1471,"score":{"minimal_stake":100000000000000,"sum_stake":100000000000000,"sum_stake_squared":10000000000000000000000000000}},
    {"address":"unsigned","round":76,"block":1499,"score":{"minimal_stake":100000000000000,"sum_stake":100000000000000,"sum_stake_squared":10000000000000000000000000000}},{"address":"unsigned","round":77,"block":1519,"score":{"minimal_stake":100000000000000,"sum_stake":100000000000000,"sum_stake_squared":10000000000000000000000000000}}]%
```

#### Get the most recent submission

```bash
$ curl "http://localhost:9999/winners"
[{"address":"0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d","round":74,"block":1468,"score":{"minimal_stake":100000000000000,"sum_stake":100000000000000,"sum_stake_squared":10000000000000000000000000000}},{"address":"0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d","round":75,"block":1488,"score":{"minimal_stake":100000000000000,"sum_stake":100000000000000,"sum_stake_squared":10000000000000000000000000000}},{"address":"unsigned","round":76,"block":1508,"score":{"minimal_stake":100000000000000,"sum_stake":100000000000000,"sum_stake_squared":10000000000000000000000000000}},{"address":"unsigned","round":77,"block":1528,"score":{"minimal_stake":100000000000000,"sum_stake":100000000000000,"sum_stake_squared":10000000000000000000000000000}}]%
```

#### Get the most recent winner

```bash
$ curl "http://localhost:9999/winners/1"
[{"address":"unsigned","round":78,"block":1548,"score":{"minimal_stake":100000000000000,"sum_stake":100000000000000,"sum_stake_squared":10000000000000000000000000000}}]%
```

#### Get all unsigned winners

```bash
$ curl "http://localhost:9999/unsigned-winners"
[{"address":"unsigned","round":76,"block":1508,"score":{"minimal_stake":100000000000000,"sum_stake":100000000000000,"sum_stake_squared":10000000000000000000000000000}},{"address":"unsigned","round":77,"block":1528,"score":{"minimal_stake":100000000000000,"sum_stake":100000000000000,"sum_stake_squared":10000000000000000000000000000}},{"address":"unsigned","round":78,"block":1548,"score":{"minimal_stake":100000000000000,"sum_stake":100000000000000,"sum_stake_squared":10000000000000000000000000000}}]%
```

#### Get the most recent unsigned winner

```bash
$ curl "http://localhost:9999/unsigned-winners/1"
[{"address":"unsigned","round":79,"block":1568,"score":{"minimal_stake":100000000000000,"sum_stake":100000000000000,"sum_stake_squared":10000000000000000000000000000}}]%
```