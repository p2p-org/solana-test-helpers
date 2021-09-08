# Solana test utilities

This is a set of solana test utilities to setup test validator, start and manage backend processes to test,
and other tools to support test environment.

This as a part of [P2P Validator](https://p2p.org) solutions.

## Usage

Add this to your `Cargo.toml`:

```toml
solana_test_helpers = "0.1.0"
solana_sdk = "1.7"
```

Now you can setup test validator like this:

```rust
use solana_test_helpers::{TestValidatorService, Program};
use solana_sdk::signature::write_keypair_file;

#[test]
fn some_test() {
    // Start test validator
    let validator = TestValidatorService::builder()
        .ledger_path("target/test-ledger")
        .build()
        .start()
        .unwrap();

    // Setup payer account
    let payer = Keypair::new();
    let payer_path = std::env::temp_dir().join("payer-keypair.json");
    write_keypair_file(&payer, &payer_path).unwrap();
    // Don't forget to airdrop some lamports to it

    // Define your program to test
    let program = Program::new("target/deploy/program.so", "target/deploy/program-keypair.json", payer_path);
    
    // Deploy your program to the test validator
    program.deploy().unwrap();
    
    // Or you can deploy your program only if it has changed since last deployment
    program.deploy_if_changed().unwrap();
    
    // Get validator JSON RPC  URL
    let rpc_url = validator.rpc_url();
    
    // Now you can run tests for the validator
}
```
