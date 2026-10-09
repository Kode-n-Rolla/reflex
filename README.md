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
- Fees, cancellation, production mint allowlists and recovery of residual incentive
  funds are intentionally deferred.

## Build

```bash
NO_DNA=1 cargo fmt --all -- --check
NO_DNA=1 cargo check
NO_DNA=1 anchor build
NO_DNA=1 cargo test --no-run -q
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

The test covers YES and NO resolution, single- and both-side positions, two stakers,
claim ordering, rounding, insufficient custody rollback, rejected repeated claims,
and rejected deposits after resolution.
