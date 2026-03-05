# Miden + Brale Stablecoin Integration Demo

Canonical integration reference for institutional custody on Miden, demonstrating
ECDSA K256 (secp256k1) signing with pluggable MPC/HSM backends and PSM multisig.

## Architecture

```
                    ┌─────────────────────┐
                    │   Brale Application  │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │    BraleKeystore     │
                    │  (TransactionAuth)   │
                    └──────────┬──────────┘
                               │
              ┌────────────────┼────────────────┐
              │                │                │
    ┌─────────▼─────┐  ┌──────▼──────┐  ┌──────▼──────┐
    │ Filesystem    │  │ ExternalSigner │  │  PSM       │
    │ KeyStore      │  │ (MPC/HSM)      │  │ MultisigClient│
    │ (key mgmt)    │  │ sign_prehash() │  │ (2-of-3)   │
    └───────────────┘  └───────────────┘  └──────┬──────┘
                                                  │
                    ┌──────────▼──────────┐
                    │     Miden Node      │
                    │   (testnet RPC)     │
                    └─────────────────────┘
```

## Signing Patterns

| Pattern | Use Case | Auth Component | Client Type |
|---------|----------|----------------|-------------|
| `BraleKeystore` | Single custodian, MPC provider | `AuthEcdsaK256Keccak` | `Client<BraleKeystore>` |
| PSM Multisig | Institutional multisig, treasury | PSM `multisig_ecdsa` | `MultisigClient` |

## Quick Start

```bash
# 1. Configure environment
cp .env.example .env
# Edit .env with your RPC endpoint and settings

# 2. Build and test
make build
make test
```

## Make Commands

### Build & Test

| Command | Description |
|---|---|
| `make check` | Type-check the workspace (`cargo check`) |
| `make build` | Build all binaries |
| `make test` | Run all 18 unit and integration tests (MockChain, no network required) |

### Self-Contained Demos

| Command | Description | Network Required |
|---|---|---|
| `make full-demo` | Alias for `full-demo-public` | Miden testnet |
| `make full-demo-public` | E2E single-signer flow with public wallets | Miden testnet |
| `make full-demo-private` | E2E single-signer flow with private wallets (`--private`) | Miden testnet |
| `make multisig-demo` | 2-of-3 ECDSA K256 multisig flow | Miden testnet + PSM server |

See [Running the Demos](docs/guide/09-running-demos.md) for step-by-step breakdown.

### Individual Operations

These require `.env` configuration. Set `ISSUER_ACCOUNT_ID` after deploying an issuer.

| Command | Description | CLI Args |
|---|---|---|
| `make create-account` | Create an ECDSA K256 wallet | — |
| `make deploy-issuer` | Deploy a faucet with ECDSA auth | `[symbol] [decimals] [max_supply]` |
| `make mint` | Mint tokens to a target account | `<target_id> <amount>` |
| `make transfer` | Transfer tokens between accounts | `<sender_id> <recipient_id> <amount>` |
| `make burn` | Burn tokens from a sender | `<sender_id> <amount>` |
| `make read-balance` | Read account token balance | `<account_id>` |
| `make read-supply` | Read total token supply | — |

## RFI Coverage

| # | Requirement | Status | Code |
|---|-------------|--------|------|
| 1 | Signer injection (MPC/HSM) | **Working** | `keystore.rs` — `ExternalSigner` trait |
| 2 | Create wallet / account | **Working** | `operations.rs` — `create_wallet_account()` |
| 3 | Read token balance | **Working** | `operations.rs` — `read_balance()` |
| 4 | Read total supply | **Working** | `operations.rs` — `read_total_supply()` |
| 5 | Deploy / register fungible token | **Working** | `operations.rs` — `deploy_issuer_account()` |
| 6 | Mint (increase supply) | **Working** | `operations.rs` — `mint_tokens()` |
| 7 | Burn (reduce supply) | **Working** | `operations.rs` — `burn_tokens()` |
| 8 | Transfer (holder to recipient) | **Working** | `operations.rs` — `transfer_tokens()` |
| 9 | Compliance controls (denylist/freeze) | **Roadmap** | [RFI §12](docs/guide/05-rfi-requirements.md#12-compliance-controls-denylist-freeze-wipe) |
| 10 | Finality signal | **Documented** | [RFI §9](docs/guide/05-rfi-requirements.md#9-finality-model) |
| 11 | Deposit detection | **Working** | `operations.rs` — `consume_notes()` + `sync_state()` |
| 12 | Fee estimation | **Roadmap** | [RFI §10](docs/guide/05-rfi-requirements.md#10-fee-estimation--simulation) |
| 13 | Account activation / opt-in | **None required** | [RFI §11](docs/guide/05-rfi-requirements.md#11-account-activation--token-opt-in) |
| 14 | Read token metadata | **Working** | `operations.rs` — `read_token_metadata()` |
| — | Signing test vectors | **Working** | [SIGNING_TEST_VECTORS.md](docs/SIGNING_TEST_VECTORS.md) |
| — | Brale ↔ Miden mapping | **Documented** | [Integration Mapping](docs/guide/08-brale-integration-mapping.md) |
| — | Auth scheme clarification | **Documented** | [Signer Injection §Auth Schemes](docs/guide/03-signer-injection.md#supported-authentication-schemes) |

## Guide

1. [Miden for Ethereum Engineers](docs/guide/01-miden-for-ethereum-engineers.md) — mental model, accounts, notes, STARK proofs, faucets, privacy, concept mapping
2. [Architecture](docs/guide/02-architecture.md) — system diagram, component inventory, signing patterns
3. [Signer Injection: MPC/HSM Bridge](docs/guide/03-signer-injection.md) — `ExternalSigner` trait, `BraleKeystore`, signing chain, production integration
4. [PSM: Private Multisig Orchestration](docs/guide/04-psm-multisig.md) — what PSM solves, `KeyManager` bridge, 2-of-3 flow
5. [RFI Requirements Walkthrough](docs/guide/05-rfi-requirements.md) — all 12 requirements mapped to working code
6. [Testing](docs/guide/06-testing.md) — `MockChain`, test inventory, running tests
7. [What's Next](docs/guide/07-whats-next.md) — compliance roadmap, production gaps, binary reference
8. [Brale ↔ Miden Integration Mapping](docs/guide/08-brale-integration-mapping.md) — concept mapping, transaction lifecycle, deposit detection
9. [Running the Demos](docs/guide/09-running-demos.md) — full-demo, multisig-demo, storage modes, PSM setup

### Reference Documents

- [SIGNING_SPEC.md](docs/SIGNING_SPEC.md) — byte-level signing protocol specification
- [SIGNING_TEST_VECTORS.md](docs/SIGNING_TEST_VECTORS.md) — deterministic test vectors for MPC integration validation

## Project Structure

```
integration/
├── src/
│   ├── lib.rs              # Module declarations
│   ├── config.rs           # Env-based configuration
│   ├── display.rs          # Table formatting helpers (balance, signer status)
│   ├── helpers.rs          # Template helpers (DO NOT MODIFY)
│   ├── keystore.rs         # BraleKeystore + ExternalSigner trait
│   ├── mock_signer.rs      # SimulatedMpcSigner for testing
│   ├── multisig.rs         # PSM KeyManager wrapper
│   └── operations.rs       # Issuer operations (deploy, mint, burn, transfer, reads)
├── src/bin/
│   ├── create_account.rs   # Create ECDSA wallet
│   ├── deploy_issuer.rs    # Deploy faucet
│   ├── mint.rs             # Mint tokens
│   ├── burn.rs             # Burn tokens
│   ├── transfer.rs         # Transfer tokens
│   ├── read_balance.rs     # Read balance
│   ├── read_supply.rs      # Read total supply
│   ├── full_demo.rs        # E2E single-signer demo
│   └── multisig_demo.rs    # 2-of-3 multisig demo
└── tests/
    ├── test_utils.rs       # Shared MockChain helpers
    ├── single_signer_test.rs   # MockChain integration tests
    └── operations_unit_test.rs # Unit tests for config, keys, signer
docs/
├── guide/
│   ├── 01-miden-for-ethereum-engineers.md
│   ├── 02-architecture.md
│   ├── 03-signer-injection.md
│   ├── 04-psm-multisig.md
│   ├── 05-rfi-requirements.md
│   ├── 06-testing.md
│   ├── 07-whats-next.md
│   ├── 08-brale-integration-mapping.md
│   └── 09-running-demos.md
├── SIGNING_SPEC.md         # Byte-level MPC signing protocol
└── SIGNING_TEST_VECTORS.md # Deterministic test vectors
```

## Configuration

All settings via environment variables or `.env` file:

| Variable | Default | Description |
|----------|---------|-------------|
| `MIDEN_RPC_ENDPOINT` | `https://rpc.testnet.miden.io` | Miden node RPC |
| `PSM_ENDPOINT` | `http://localhost:50051` | PSM server (for multisig) |
| `SQLITE_STORE_PATH` | `./store.sqlite3` | Client state database |
| `KEYSTORE_PATH` | `./keystore` | Filesystem key storage |
| `ISSUER_ACCOUNT_ID` | — | Faucet ID (set after deploy) |
| `LOG_LEVEL` | `info` | Tracing log level |

## PSM Server Setup (for multisig)

```bash
# Clone the PSM repo
git clone https://github.com/OpenZeppelin/private-state-manager

# Run the server (defaults write to /var/psm which requires root — use local paths)
cd private-state-manager
PSM_STORAGE_PATH=./data/storage PSM_METADATA_PATH=./data/metadata PSM_KEYSTORE_PATH=./data/keystore cargo run -p private-state-manager-server
# gRPC: localhost:50051, HTTP: localhost:3000
```

## Compliance

This demo does **not** enforce denylist/freeze, OFAC screening, KYC/AML, or
geographic restrictions. See [RFI §12](docs/guide/05-rfi-requirements.md#12-compliance-controls-denylist-freeze-wipe)
for details and timeline.

## Dependencies

- Miden SDK v0.13 (miden-client, miden-protocol, miden-standards, miden-testing)
- PSM v0.13.0 (miden-multisig-client from crates.io)
- Rust nightly-2025-12-10
