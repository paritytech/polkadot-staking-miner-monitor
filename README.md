## Polkadot staking miner monitor

This is a simple tool that monitors each election in polkadot-sdk-based chains and
then stores the following data related in a postgres database:
- submissions: The list of all submissions in each election, this is regarded 
  as successful if the solution extrinsic is accepted by the chain. The solution may
  be rejected at the end of the election when it's fully verified. You need check `slashed`
  together with this to know whether a solution was truly valid.
- elections: Get the results of each election, which may be a signed solution, an unsigned solution or a failed election.
- slashed: Get the slashed accounts

The tool is based on the subxt library and is written in Rust.

## Web APIs
- `GET /docs/` - swagger UI
- `GET /docs/openapi.json` - OpenAPI JSON schema
- `GET /docs/openapi.yaml` - OpenAPI YAML schema
- `GET /submissions/` - Get all submissions from the database in JSON format.
- `GET /submissions/success` - Get all successful submissions from the database in JSON format.
- `GET /submissions/failed` - Get all failed submissions from the database in JSON format.
- `GET /submissions/{n}` - Get the `n` most recent submissions from the database in JSON format, n is a number.
- `GET /elections/` - Dump all elections from the database in JSON format.
- `GET /elections/{n}` - Get the `n` most recent winners from the database in JSON format, n is a number.
- `GET /elections/signed` - Dump all elections that were completed based on signed solutions.
- `GET /elections/unsigned` - Dump all elections that were completed based on unsigned solutions.
- `GET /elections/failed` - Dump all failed elections.
- `GET /slashed/` - Get all slashed solutions from the database in JSON format.
- `GET /slashed/{n}` - Get the `n` most recent slashed solutions from the database in JSON format, n is a number.
- `GET /metrics` - Fetch prometheus metrics.
- `GET /stats` - Fetch stats which include the total number of submissions, elections and slashed solutions.

## Roadmap

1. Add functionality to start syncing from a specific block instead of the latest. To get the full history of the chain.
2. More sophisticated API to get more detailed information about the submissions and the winners.
3. Support older metadata versions.

### Limitations

This tool is based on subxt and this means that it is limited to blocks with metadata v14
or above. This is why full history is not supported.

### Usage

```bash
$ cargo run --release -- --polkadot wss://rpc.polkadot.io --postgres "postgresql://user:pwd@localhost/polkadot"
```

Open another terminal and run the following commands to use the API:

#### Get all submissions

```bash
$ curl "http://localhost:9999/submissions" | jq
[
  {
    "who": "0x8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48",
    "round": 54,
    "block": 1059,
    "score": {
      "minimal_stake": 100000000000000,
      "sum_stake": 100000000000000,
      "sum_stake_squared": 10000000000000000000000000000
    },
    "success": true
  },
  {
    "who": "unsigned",
    "round": 55,
    "block": 1087,
    "score": {
      "minimal_stake": 100000000000000,
      "sum_stake": 100000000000000,
      "sum_stake_squared": 10000000000000000000000000000
    },
    "success": true
  }
]
```

#### Get the most recent submission
```bash
$ curl "http://localhost:9999/submissions/1" | jq
[
  {
    "who": "unsigned",
    "round": 57,
    "block": 1127,
    "score": {
      "minimal_stake": 100000000000000,
      "sum_stake": 100000000000000,
      "sum_stake_squared": 10000000000000000000000000000
    },
    "success": true
  }
]
```

#### Get all elections

```bash
$ curl "http://localhost:9999/elections" | jq
[
  {
    "result": "signed",
    "who": "0x8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48",
    "round": 55,
    "block": 1076,
    "score": {
      "minimal_stake": 100000000000000,
      "sum_stake": 100000000000000,
      "sum_stake_squared": 10000000000000000000000000000
    }
  },
  {
    "result": "unsigned",
    "who": null,
    "round": 56,
    "block": 1096,
    "score": {
      "minimal_stake": 100000000000000,
      "sum_stake": 100000000000000,
      "sum_stake_squared": 10000000000000000000000000000
    }
  }
]
```

#### Get the most recent election

```bash
$ curl "http://localhost:9999/elections/1"
[
  {
    "result": "unsigned",
    "who": null,
    "round": 57,
    "block": 1116,
    "score": {
      "minimal_stake": 100000000000000,
      "sum_stake": 100000000000000,
      "sum_stake_squared": 10000000000000000000000000000
    }
  }
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

#### Get stats

```bash
$ curl localhost:9999/stats | jq
{
  "submissions": {
    "total": 188,
    "failed": 1,
    "success": 187
  },
  "elections": {
    "total": 177,
    "failed": 0,
    "signed": 12,
    "unsigned": 165
  },
  "slashed": 0
}
```

### Database migrations

This tool has a simple database with three tables: `submissions`, `elections` and `slashed` which is located in the `migrations` folder.
To add a new migration, just create a new file with the following format: `V{version}__{description}.sql` and it will be automatically applied when the tool is started.
