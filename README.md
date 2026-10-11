# Reflex

Reflex is an Anchor program that creates an incentive Pool for a mock prediction
market. Stakers deposit YES or NO outcome tokens, accumulate stake-seconds, and
claim both principals after resolution. Incentive rewards are distributed only from
the winning side and rounded down.

## Trust model

- Market accounts use a local 320-byte mock layout with YES and NO mint keys at the
  offsets declared in `constants.rs`.
- `Config.authority` selects the YES or NO result through `resolve_pool`.
- This is not a verified integration with the World/prediCt mainnet program.
- A creator/briber fee is charged on outcome deposits; protocol fees remain disabled.
- Cancellation and recovery of residual incentive funds are intentionally deferred.
- Production builds accept canonical mainnet USDC and WSOL incentive mints. Local
  mock incentive mints require an explicit `local-test` build and creator authority.

## Devnet deployment

Reflex is deployed to devnet at `7rQ3UP2xNt173iqHodzJDto9RXtZALFrdeij8aoH41hv`.

## Build

```bash
NO_DNA=1 cargo fmt --all -- --check
NO_DNA=1 cargo check
NO_DNA=1 anchor build
NO_DNA=1 cargo test --no-run -q
```

For local mock-mint integration tests, build the deployed artifact explicitly with:

```bash
NO_DNA=1 anchor build -- --features local-test
```

## Local integration test

Start Surfpool and wait for the deployment runbook to finish:

```bash
NO_DNA=1 surfpool start --offline --ci --yes
```

In another terminal:

```bash
ANCHOR_WALLET="$HOME/.config/solana/id.json" \
  NO_DNA=1 cargo test -p tests test_reflex_lifecycle -- --nocapture
```

The test covers incentive-mint validation, YES/NO fee accounting, creator fee
withdrawal, YES and NO resolution, single- and both-side positions, two stakers,
claim ordering, rounding, insufficient custody rollback, rejected repeated claims,
and rejected deposits after resolution.

---

# Test Output

The tests were run on localnet because devnet is not yet supported by the mock-mint integration test and World on devnet. The test output shows the expected behavior of the incentive pool, including the correct distribution of rewards and the handling of edge cases.

![Test 1](/public/image.png)
![Test 2](/public/image-1.png)

## Architecture Diagram - Overview

![Architecture Diagram](/public/diagram%20overview.png)
