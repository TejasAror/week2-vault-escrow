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
```

# 1. Vault Program

   The Vault program allows a user to initialize a PDA controlled SOL vault, deposit SOL, withdraw SOL and close the vault.

   Vault PDA Architecture

   The vault PDA is derived using:

        ["vault", user]

   The state PDA is derived using:

         ["state", user]

  The vault PDA is controlled by the program and uses PDA seeds when signing for withdrawals.

   Vault State

   The vault stores:

        vault_bump
        state_bump

  These bumps are used to work with the corresponding PDAs.

  ## Vault Instructions
  
  initialize

  Creates the user's vault state and initializes the vault PDA with enough lamports to satisfy rent-exemption requirements.

      deposit(amount)
      
  Transfers SOL from the user into the PDA vault.

  Validation includes:

   Amount must be greater than zero.
   The vault must already be initialized.

         withdraw(amount)

   Withdraws SOL from the PDA vault back to the user.

   The program signs for the vault PDA using its PDA seeds.

   The implementation also ensures that the vault does not fall below the required rent exempt balance.

            close

  Closes the vault and state accounts.

   The remaining vault lamports are returned to the user, and the state account is closed.


 # 2. Escrow Program

  The Escrow program implements a token-for-token exchange between a maker and a taker.

   The maker deposits Token A into an escrow-controlled token account and specifies how much Token B they want in return.

  The taker can accept the terms by paying Token B and receiving Token A.


 # Escrow PDA Architecture

  The escrow PDA is derived using:

       ["escrow", maker, seed].

  The seed allows a maker to create separate escrow agreements.

   The escrow state stores:

               seed
               maker
               mint_a
               mint_b
               receive
               bump

   Where:

   seed identifies the escrow.
   maker identifies the creator.
   mint_a is the token deposited by the maker.
   mint_b is the token requested from the taker.
   receive is the required amount of Token B.
   bump is the escrow PDA bump.

   The escrow PDA controls the Token A vault.


# Escrow Instructions

          make(seed, deposit, receive)

  Creates a new escrow agreement.

   The maker:

   Creates the escrow PDA.
   Creates the escrow-controlled Token A vault.
   Deposits Token A into the vault.
   Defines the amount of Token B required from the taker.

   The terms are stored on-chain.

         take


    Allows a taker to accept the escrow.

   The taker:

   Pays the maker the requested amount of Token B.
   Receives the deposited Token A.
   The escrow Token A vault is closed.
   The escrow state account is closed.

   The transaction uses the escrow PDA as the authority for the Token A vault.

   The implementation ensures the taker cannot be the maker.

               refund


  Allows the maker to cancel the escrow.

  The maker:

   Recovers the deposited Token A.
   The Token A vault is closed.
   The escrow state account is closed.

   Only the maker can perform the refund.


            update(receive)


  Allows the maker to update the requested Token B amount before the escrow is completed.

   The deposited Token A remains unchanged.

  Only the maker can update the escrow terms.
