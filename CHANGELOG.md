# Changelog

The format is based on [Keep a Changelog].

[Keep a Changelog]: http://keepachangelog.com/en/1.0.0/

## [v0.1.0-alpha] - 2024-11-12

This is the first elease of the `polkadot staking miner monitoring tool`, which provides a REST API to get historical information about the elections, submissions, and slashed solutions in the polkadot network.

The major reason for this tool is to provide a way to monitor the election status in the polkadot network, which can be connected with other monitoring tools like prometheus and Grafana.

Currently it has some restrictions, like it's only possible to start syncing from the latest block, and it only supports metadata v14 or above. This will be improved in future versions.

Beware that this software is an alpha version and it may contain bugs.
