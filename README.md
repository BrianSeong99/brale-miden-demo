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

# 2. Build
make build

# 3. Run individual operations
make create-account      # Create ECDSA K256 wallet
make deploy-issuer       # Deploy faucet with ECDSA auth
make mint                # Mint tokens (set ISSUER_ACCOUNT_ID first)
make transfer            # Transfer tokens between accounts
make burn                # Burn tokens
make read-balance        # Read account balance
make read-supply         # Read total supply

# 4. Run full E2E demo
make full-demo

# 5. Run multisig demo (requires PSM server)
make multisig-demo

# 6. Run tests
make test
```

## Project Structure

```
integration/
├── src/
│   ├── lib.rs              # Module declarations
│   ├── config.rs           # Env-based configuration
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
├── SIGNING_SPEC.md         # Byte-level MPC signing protocol
└── COMPLIANCE_ROADMAP.md   # Compliance limitations and roadmap
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

# Run the server
cd private-state-manager
cargo run -p private-state-manager-server
# Default: http://localhost:50051
```

## Compliance

This demo does **not** enforce denylist/freeze, OFAC screening, KYC/AML, or
geographic restrictions. See [docs/COMPLIANCE_ROADMAP.md](docs/COMPLIANCE_ROADMAP.md)
for details and timeline.

## Dependencies

- Miden SDK v0.13 (miden-client, miden-protocol, miden-standards, miden-testing)
- PSM v0.13.0 (miden-multisig-client, pinned to local path)
- Rust nightly-2025-12-10
