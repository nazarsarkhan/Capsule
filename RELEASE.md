# Capsule Alpha Release Guide

This repository is prepared for a first public GitHub release. Do not publish automatically from automation until the checklist below is complete.

## Release Checklist

Run from the repository root:

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
cargo build --release
capsule doctor
capsule run demo-python/app.py --yes
capsule run demo-node/index.js --yes
capsule run demo-ts/app.ts --yes
```

On Windows, if incremental compilation reports a file-locking warning:

```powershell
$env:CARGO_INCREMENTAL=0
cargo build --release
```

## Manual Smoke Tests

```bash
capsule doctor
capsule inspect demo-ts/app.ts
capsule run demo-python/app.py --yes
capsule run demo-node/index.js --yes
capsule run demo-ts/app.ts --yes
```

Confirm the demo projects do not contain:

```text
.venv/
node_modules/
```

## GitHub Release Steps

1. Ensure `README.md`, `CHANGELOG.md`, `LICENSE`, and this file are current.
2. Run the release checklist.
3. Commit the release prep changes.
4. Tag the release:

   ```bash
   git tag v0.1.0-alpha.1
   git push origin main --tags
   ```

5. Build the release binary:

   ```bash
   cargo build --release
   ```

6. Upload `target/release/capsule.exe` to GitHub Releases.
7. Use the changelog entry as the release notes.
8. Do not mark Alpha as stable; call out the current limitations.
