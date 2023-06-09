## Staking miner monitor

This is a simple tool that monitors each election
and emits prometheus metrics for each submission and
which solution that was rewarded.

## Usage

```bash
$ curl http://127.0.0.1:9999/metrics
# HELP epm_election_winner EPM election winner per round
# TYPE epm_election_winner counter
epm_election_winner{address="0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d",round="92",score="{\"score_minimal_stake\":\"100000000000000\",\"score_sum_squared\":\"10000000000000000000000000000\",\"score_sum_stake\":\"100000000000000\"}"} 1
epm_election_winner{address="0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d",round="93",score="{\"score_minimal_stake\":\"100000000000000\",\"score_sum_squared\":\"10000000000000000000000000000\",\"score_sum_stake\":\"100000000000000\"}"} 1
epm_election_winner{address="0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d",round="94",score="{\"score_minimal_stake\":\"100000000000000\",\"score_sum_squared\":\"10000000000000000000000000000\",\"score_sum_stake\":\"100000000000000\"}"} 1
epm_election_winner{address="0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d",round="95",score="{\"score_minimal_stake\":\"100000000000000\",\"score_sum_squared\":\"10000000000000000000000000000\",\"score_sum_stake\":\"100000000000000\"}"} 1
epm_election_winner{address="0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d",round="96",score="{\"score_minimal_stake\":\"100000000000000\",\"score_sum_squared\":\"10000000000000000000000000000\",\"score_sum_stake\":\"100000000000000\"}"} 1
epm_election_winner{address="0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d",round="97",score="{\"score_minimal_stake\":\"100000000000000\",\"score_sum_squared\":\"10000000000000000000000000000\",\"score_sum_stake\":\"100000000000000\"}"} 1
epm_election_winner{address="0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d",round="98",score="{\"score_minimal_stake\":\"100000000000000\",\"score_sum_squared\":\"10000000000000000000000000000\",\"score_sum_stake\":\"100000000000000\"}"} 1
epm_election_winner{address="0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d",round="99",score="{\"score_minimal_stake\":\"100000000000000\",\"score_sum_squared\":\"10000000000000000000000000000\",\"score_sum_stake\":\"100000000000000\"}"} 1
# HELP epm_submissions EPM submissions per round
# TYPE epm_submissions counter
epm_submissions{address="0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d",round="100",score="{\"score_minimal_stake\":\"100000000000000\",\"score_sum_squared\":\"10000000000000000000000000000\",\"score_sum_stake\":\"100000000000000\"}"} 1
epm_submissions{address="0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d",round="92",score="{\"score_minimal_stake\":\"100000000000000\",\"score_sum_squared\":\"10000000000000000000000000000\",\"score_sum_stake\":\"100000000000000\"}"} 1
epm_submissions{address="0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d",round="93",score="{\"score_minimal_stake\":\"100000000000000\",\"score_sum_squared\":\"10000000000000000000000000000\",\"score_sum_stake\":\"100000000000000\"}"} 1
epm_submissions{address="0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d",round="94",score="{\"score_minimal_stake\":\"100000000000000\",\"score_sum_squared\":\"10000000000000000000000000000\",\"score_sum_stake\":\"100000000000000\"}"} 1
epm_submissions{address="0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d",round="95",score="{\"score_minimal_stake\":\"100000000000000\",\"score_sum_squared\":\"10000000000000000000000000000\",\"score_sum_stake\":\"100000000000000\"}"} 1
epm_submissions{address="0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d",round="96",score="{\"score_minimal_stake\":\"100000000000000\",\"score_sum_squared\":\"10000000000000000000000000000\",\"score_sum_stake\":\"100000000000000\"}"} 1
epm_submissions{address="0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d",round="97",score="{\"score_minimal_stake\":\"100000000000000\",\"score_sum_squared\":\"10000000000000000000000000000\",\"score_sum_stake\":\"100000000000000\"}"} 1
epm_submissions{address="0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d",round="98",score="{\"score_minimal_stake\":\"100000000000000\",\"score_sum_squared\":\"10000000000000000000000000000\",\"score_sum_stake\":\"100000000000000\"}"} 1
epm_submissions{address="0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d",round="99",score="{\"score_minimal_stake\":\"100000000000000\",\"score_sum_squared\":\"10000000000000000000000000000\",\"score_sum_stake\":\"100000000000000\"}"} 1
```
