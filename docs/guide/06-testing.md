# Testing

[Back to Index](../WALKTHROUGH.md) | [Previous: RFI Requirements](05-rfi-requirements.md)

---

## Running Tests

```bash
make test
# Runs all 16 tests — no network connection required
```

Tests use `MockChain` from `miden-testing`, a deterministic offline Miden execution
environment. It simulates the full transaction lifecycle (build, execute, prove,
apply delta) without connecting to any network.

## How MockChain Works

```rust
// Build a test environment
let mut builder = MockChain::builder();
let faucet = builder.add_existing_basic_faucet(
    Auth::EcdsaK256KeccakAuth, "TST", 1_000_000, None,
)?;
let wallet = builder.add_existing_wallet(Auth::EcdsaK256KeccakAuth)?;
let note = builder.add_p2id_note(
    faucet.id(), wallet.id(), &[asset.into()], NoteType::Private,
)?;
let mock_chain = builder.build()?;

// Execute a transaction
let executed_tx = mock_chain
    .build_tx_context(wallet.id(), &[note.id()], &[])?
    .build()?
    .execute()
    .await?;

// Apply state changes and verify
wallet.apply_delta(executed_tx.account_delta())?;
assert_eq!(wallet.vault().get_balance(faucet.id())?, 100);
```

`MockChain` provides:
- Deterministic block production (no timing dependencies)
- Built-in ECDSA K256 authentication (via `Auth::EcdsaK256KeccakAuth`)
- P2ID note creation via `add_p2id_note()`
- Transaction context building with input notes

## Test Inventory

### Integration Tests (`tests/single_signer_test.rs`)

| Test | What It Verifies |
|------|-----------------|
| `create_ecdsa_wallet` | Account creation with ECDSA K256 auth succeeds |
| `deploy_issuer_account` | Faucet account created with correct type |
| `mint_and_consume` | Mint 100 tokens, consume at target, balance = 100 |
| `read_balance_after_mint` | Balance reads correctly via vault after consumption |
| `faucet_sysdata_slot_readable` | Total issuance slot accessible, starts at 0 |
| `faucet_is_configured_correctly` | Faucet structure and storage initialized properly |
| `consume_multiple_notes` | Two notes consumed in one tx, balance aggregated |

### Unit Tests (`tests/operations_unit_test.rs`)

| Test | What It Verifies |
|------|-----------------|
| `config_custom_values` | Environment variable overrides work |
| `config_has_expected_field_names` | Config struct has all required fields |
| `ecdsa_key_generation` | ECDSA K256 keypair can be generated |
| `commitment_from_public_key_deterministic` | Same key produces same commitment |
| `mock_signer_sign_returns_65_bytes` | Signature is exactly 65 bytes (r+s+v) |
| `mock_signer_deterministic` | Same key + message produces same signature (RFC 6979) |
| `mock_signer_unknown_key_fails` | Signing with unregistered key returns error |
| `keccak256_hash_word_deterministic` | Same input produces same hash |
| `keccak256_hash_word_different_inputs` | Different inputs produce different hashes |

---

[Next: What's Next](07-whats-next.md) | [Back to Index](../WALKTHROUGH.md)
