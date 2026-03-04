# Compliance Roadmap

## What This Demo Enforces

- ECDSA K256 (secp256k1) transaction authentication
- Account-level authorization via `AuthEcdsaK256Keccak` component
- Multisig threshold authorization (2-of-3, configurable) via PSM
- Transaction signing through pluggable external backends (MPC, HSM)

## What This Demo Does NOT Enforce

The following compliance controls are **not implemented** in this demo.
They are either in development at OpenZeppelin or require protocol-level changes.

### Denylist / Freeze

**Status**: In development at OpenZeppelin.
**Reference**: [OZ Miden Confidential Contracts #39](https://github.com/OpenZeppelin/miden-confidential-contracts/discussions/39)

- No on-chain denylist for accounts or addresses
- No ability to freeze individual account balances
- No compliance oracle integration

### OFAC Screening

- No OFAC/SDN list screening at the protocol level
- Screening must be implemented at the application layer before transaction submission

### Supply Caps

- Max supply is enforced at the faucet level (`BasicFungibleFaucet` max_supply parameter)
- No protocol-level aggregate supply caps across multiple issuers

### KYC/AML

- No on-chain identity verification
- KYC/AML must be enforced at the application layer
- Account creation is permissionless

### Geographic Restrictions

- No on-chain geographic restriction enforcement
- Must be implemented at the application/gateway layer

## Timeline

| Control | Expected | Source |
|---------|----------|--------|
| Denylist/freeze | TBD | OZ Confidential Contracts |
| Compliance oracle | TBD | Protocol enhancement |
| Account-level permissions | Available now | `AuthEcdsaK256KeccakAcl` component |

## Recommendations for Production

1. Implement OFAC screening in the application layer before submitting transactions
2. Use `AuthEcdsaK256KeccakAcl` for procedure-level access control when available
3. Monitor OZ Confidential Contracts for denylist/freeze support
4. Maintain an off-chain compliance database mapping account IDs to verified identities
5. Consider implementing withdrawal limits and velocity checks in the application layer
