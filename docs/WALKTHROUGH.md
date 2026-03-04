# Miden + Brale: Regulated Stablecoin Issuance on Miden

Technical walkthrough for the Brale integration demo. This document explains how Miden
works (for engineers coming from Ethereum), how each RFI requirement is implemented in
working code, and how PSM enables institutional multisig custody.

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
| 9 | Compliance controls (denylist/freeze) | **Roadmap** | [COMPLIANCE_ROADMAP.md](COMPLIANCE_ROADMAP.md) |
| 10 | Finality signal | **Documented** | [RFI Requirements §9](guide/05-rfi-requirements.md#9-finality-model) |
| 11 | Deposit detection | **Working** | `operations.rs` — `consume_notes()` + `sync_state()` |
| 12 | Fee estimation | **Roadmap** | [RFI Requirements §10](guide/05-rfi-requirements.md#10-fee-estimation--simulation) |
| 13 | Account activation / opt-in | **None required** | [RFI Requirements §11](guide/05-rfi-requirements.md#11-account-activation--token-opt-in) |

## Quick Start

```bash
cp .env.example .env          # configure endpoints
make build                    # compile everything
make test                     # run 16 tests (no network needed)
make full-demo                # E2E: deploy → mint → transfer → burn
```

## Guide

1. [Miden for Ethereum Engineers](guide/01-miden-for-ethereum-engineers.md) — mental model, accounts, notes, STARK proofs, faucets, privacy, concept mapping
2. [Architecture](guide/02-architecture.md) — system diagram, component inventory, signing patterns
3. [Signer Injection: MPC/HSM Bridge](guide/03-signer-injection.md) — `ExternalSigner` trait, `BraleKeystore`, signing chain, production integration
4. [PSM: Private Multisig Orchestration](guide/04-psm-multisig.md) — what PSM solves, `KeyManager` bridge, 2-of-3 flow, running the demo
5. [RFI Requirements Walkthrough](guide/05-rfi-requirements.md) — all 12 requirements mapped to working code
6. [Testing](guide/06-testing.md) — `MockChain`, test inventory, running tests
7. [What's Next](guide/07-whats-next.md) — compliance roadmap, production gaps, appendices

## Related Documents

- [SIGNING_SPEC.md](SIGNING_SPEC.md) — byte-level signing protocol specification
- [COMPLIANCE_ROADMAP.md](COMPLIANCE_ROADMAP.md) — compliance controls timeline and design
