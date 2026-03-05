# What's Next

[Back to Guide Index](../../README.md#guide) | [Previous: Testing](06-testing.md) | [Next: Brale Integration Mapping](08-brale-integration-mapping.md)

---

## Compliance Controls

The highest priority gap. OpenZeppelin is building denylist/freeze/wipe capabilities
as part of the Miden Confidential Contracts project:

| Control | Expected Timeline | Reference |
|---------|-------------------|-----------|
| Denylist / freeze | TBD (originally targeted v0.14) | [OZ Discussion #39](https://github.com/OpenZeppelin/miden-confidential-contracts/discussions/39) |
| Compliance callbacks | TBD | [Protocol #2432](https://github.com/0xMiden/protocol/issues/2432) |
| Audit of compliance contracts | TBD | OpenZeppelin, Trail of Bits |

Until on-chain controls are available, OFAC screening should be implemented at the
application layer before submitting transactions.

## Production Readiness

| Gap | Status | Workaround |
|-----|--------|------------|
| Fee estimation API | In development | Fees are effectively zero currently |
| Deposit webhooks | Not yet available | Poll via `sync_state()` every ~3 seconds |
| Transaction status polling | Limited | Use expiration block + timeout |
| "Pay in any token" fees | Planned | Not needed while fees are zero |
| Mainnet | TBD | Testnet and devnet available now |

## Frontend Demo

A web-based coordinator UI (forked from the
[0xMiden/MultiSig coordinator frontend](https://github.com/0xMiden/MultiSig)) is
planned. It will demonstrate:
- Issuer operations (deploy, mint, burn, supply dashboard)
- Multisig proposal and approval workflows
- WASM Miden SDK running in the browser
- PSM integration for multi-party signing

---

## Appendix A: Configuration Reference

See [README § Configuration](../../README.md#configuration).

## Appendix B: Binary Reference

| Binary | Command | Purpose |
|--------|---------|---------|
| `create_account` | `make create-account` | Create ECDSA K256 wallet |
| `deploy_issuer` | `make deploy-issuer` | Deploy faucet with ECDSA auth |
| `mint` | `make mint` | Mint tokens to target account |
| `burn` | `make burn` | Burn tokens (reduce supply) |
| `transfer` | `make transfer` | P2ID transfer between accounts |
| `read_balance` | `make read-balance` | Read account balance |
| `read_supply` | `make read-supply` | Read total supply |
| `full_demo` | `make full-demo` | Full E2E lifecycle demo (alias for `full-demo-public`) |
| `full_demo` | `make full-demo-public` | E2E demo with public wallet storage |
| `full_demo` | `make full-demo-private` | E2E demo with private wallet storage (`--private`) |
| `multisig_demo` | `make multisig-demo` | 2-of-3 multisig demo (requires PSM) |

## Appendix C: Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `miden-client` | 0.13 | Miden SDK (accounts, transactions, RPC) |
| `miden-protocol` | 0.13 | Protocol types (AccountId, Note, etc.) |
| `miden-standards` | 0.13 | Standard components (BasicWallet, BasicFungibleFaucet) |
| `miden-testing` | 0.13 | MockChain for deterministic testing |
| `miden-tx` | 0.13 | Transaction execution and authentication |
| `miden-multisig-client` | 0.13.0 (git) | PSM multisig orchestration |
| `tokio` | 1.40 | Async runtime |
| `sha3` | 0.10 | Keccak256 hashing |
| Rust toolchain | `nightly-2025-12-10` | Required by Miden SDK |

---

[Back to Guide Index](../../README.md#guide)
