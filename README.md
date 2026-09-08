# Solana Vault & Escrow

A Solana Anchor implementation of two on-chain programs:

- **Vault** — a PDA-controlled SOL/lamports vault.
- **Escrow** — an SPL Token A/B escrow with maker-controlled terms.

This project was built as part of the **Q3 2026 Solana Builders Cohort — Week 2**.

---

## Overview

The project demonstrates core Solana program-development concepts using the Anchor framework:

- Program Derived Addresses (PDAs)
- PDA signing with seeds
- SOL/lamport transfers
- SPL Token transfers
- Associated Token Accounts (ATAs)
- Token account ownership and authority
- Anchor account constraints
- Account initialization and closure
- Error handling and authorization
- LiteSVM-based integration testing

The repository contains two independent Anchor programs:

```text
week2-vault-escrow/
├── programs/
│   ├── q3_26_vault/
│   │   ├── src/
│   │   └── tests/
│   │       └── vault.rs
│   │
│   └── escrowq32026/
│       ├── src/
│       └── tests/
│           └── escrow.rs
│
├── Anchor.toml
├── Cargo.toml
├── Cargo.lock
└── README.md
