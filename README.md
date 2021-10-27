[![codecov](https://codecov.io/gh/Anchor-Protocol/anchor-token-contracts/branch/main/graph/badge.svg?token=NK4H00P3KH)](https://codecov.io/gh/Anchor-Protocol/anchor-token-contracts)

# Anchor Token (ANC) Contracts
This monorepository contains the source code for the Money Market smart contracts implementing Anchor Protocol on the [Terra](https://terra.money) blockchain.

You can find information about the architecture, usage, and function of the smart contracts on the official Anchor documentation [site](https://docs.anchorprotocol.com/smart-contracts/anchor-token).

### Dependencies

Anchor Token depends on [Terraswap](https://terraswap.io) and uses its [implementation](https://github.com/terraswap/terraswap) of the CW20 token specification.


## Contracts

| Contract                                 | Reference                                                                                         | Description                                                                    |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| [`airdrop`](./contracts/airdrop)         | [doc](https://docs.anchorprotocol.com/smart-contracts/anchor-token/airdrop)   | Holds ANC tokens which are to be used Luna staker incentives                   |
| [`collector`](./contracts/collector)     | [doc](https://docs.anchorprotocol.com/smart-contracts/anchor-token/collector) | Accumulates protocol fees, converts them to ANC and distributes to ANC stakers |
| [`community`](../contracts/community)    | [doc](https://docs.anchorprotocol.com/smart-contracts/anchor-token/community) | Manages ANC community grants                                                   |
| [`distributor`](./contracts/distributor) | [doc](https://docs.anchorprotocol.com/smart-contracts/anchor-token/dripper)   | Holds ANC tokens which are to be used as borrower incentives                   |
| [`gov`](./contracts/gov)                 | [doc](https://docs.anchorprotocol.com/smart-contracts/anchor-token/gov)       | Handles Anchor Governance and reward distribution to ANC stakers               |
| [`staking`](./contracts/staking)         | [doc](https://docs.anchorprotocol.com/smart-contracts/anchor-token/staking)   | Handles ANC-UST pair LP token staking                                          |
| [`vesting`](./contracts/vesting)         | [doc](https://docs.anchorprotocol.com/smart-contracts/anchor-token/vesting)   | Holds ANC tokens which are to be used ANC token allocation vesting             |

## Development

### Environment Setup

- Rust v1.44.1+
- `wasm32-unknown-unknown` target
- Docker

1. Install `rustup` via https://rustup.rs/

2. Run the following:

```sh
rustup default stable
rustup target add wasm32-unknown-unknown
```

3. Make sure [Docker](https://www.docker.com/) is installed

### Unit / Integration Tests

Each contract contains Rust unit and integration tests embedded within the contract source directories. You can run:

```sh
cargo unit-test
cargo integration-test
```

### Compiling

After making sure tests pass, you can compile each contract with the following:

```sh
RUSTFLAGS='-C link-arg=-s' cargo wasm
cp ../../target/wasm32-unknown-unknown/release/cw1_subkeys.wasm .
ls -l cw1_subkeys.wasm
sha256sum cw1_subkeys.wasm
```

#### Production

For production builds, run the following:

```sh
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/workspace-optimizer:0.11.5
```

This performs several optimizations which can significantly reduce the final size of the contract binaries, which will be available inside the `artifacts/` directory.

## License

Copyright 2020 Anchor Protocol

Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except in compliance with the License. You may obtain a copy of the License at http://www.apache.org/licenses/LICENSE-2.0. Unless required by applicable law or agreed to in writing, software distributed under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.

See the License for the specific language governing permissions and limitations under the License.
