# Capsule

Capsule Alpha is a zero-setup dev runtime for Python, Node.js, and TypeScript.

Run a script without manually creating a virtual environment, installing npm packages in the project, or remembering the right setup command:

```bash
capsule run app.py
capsule run index.js
capsule run app.ts
```

No `.venv`. No `node_modules`. No setup.

Capsule installs dependencies into a global cache and runs your code without modifying your project.

## Why Capsule Exists

Small scripts and demos often need dependencies before they need full project setup. Capsule provides one transparent command that:

- detects Python, Node.js, or TypeScript
- scans direct imports
- creates an isolated global cached environment
- installs dependencies into that cache
- writes exact versions to `capsule.lock`
- runs the file or project with clear logs

Capsule is not trying to replace `uv`, `pip`, `npm`, or `tsx`. It orchestrates them behind a minimal developer experience.

## Supported Languages

Capsule Alpha supports only:

- Python
- Node.js
- TypeScript

Go, Rust, and Ruby are roadmap items and are not active in Alpha.

## Install

### From GitHub Source

```bash
git clone https://github.com/USER/capsule
cd capsule
cargo install --path .
```

### From GitHub Directly

```bash
cargo install --git https://github.com/USER/capsule
```

### From Release Binary

1. Download `capsule.exe` from GitHub Releases.
2. Add its folder to `PATH`.
3. Verify:

```bash
capsule doctor
```

## Windows Notes

- Building from source with the default MSVC Rust toolchain requires Visual Studio Build Tools with the "Desktop development with C++" workload.
- Python should be available through the Python launcher (`py`).
- Node.js and npm must be installed for Node.js and TypeScript support.

## Quickstart

Python:

```bash
echo 'import requests; print(requests.__version__)' > app.py
capsule run app.py --yes
```

Node.js:

```bash
echo 'const express = require("express"); console.log("ok")' > index.js
capsule run index.js --yes
```

TypeScript:

```bash
echo 'import { z } from "zod"; console.log(z.string().parse("ok"))' > app.ts
capsule run app.ts --yes
```

## Commands

Run a file or project:

```bash
capsule run app.py
capsule run index.js
capsule run app.ts
capsule run .
```

Scan direct imports without installing:

```bash
capsule scan app.py
capsule scan .
```

Inspect detection, cache path, and lockfile state without installing:

```bash
capsule inspect app.ts
```

Create or update `capsule.lock` without running:

```bash
capsule lock app.py --yes
```

Check local tools:

```bash
capsule doctor
```

Clean Capsule caches:

```bash
capsule clean --all
```

## Cache Layout

Capsule stores runtime state under:

```text
~/.capsule/
  python/envs/<project_hash>/
  node/envs/<project_hash>/
  typescript/envs/<project_hash>/
```

Node.js and TypeScript dependencies are installed into hidden cached `node_modules` directories, not into your project.

## Lockfile

Capsule writes `capsule.lock` with exact direct dependency versions:

```json
{
  "version": 1,
  "project_hash": "abc123",
  "language": "python",
  "runtime": {
    "name": "py -3",
    "version": "3.14.3"
  },
  "packages": {
    "requests": {
      "version": "2.32.5"
    }
  }
}
```

If `capsule.lock` exists, Capsule respects pinned versions for currently scanned direct dependencies.

## Verified on Windows

During Alpha development, the main manual verification flow has been Windows:

```powershell
capsule doctor
capsule run app.py --yes
capsule run index.js --yes
capsule run app.ts --yes
```

## Limitations

- Alpha quality: behavior is intentionally conservative and still evolving.
- Capsule scans direct dependencies only.
- Python scanning is MVP-level parser logic, not a full Python AST parser.
- Node.js and TypeScript scanning cover common `import` and `require` forms, not every dynamic import pattern.
- TypeScript single-file support is strongest; project-level TypeScript support is limited to basic `dev` or `start` scripts.
- Capsule uses npm for Node.js and TypeScript hidden environments.
- Tested mainly on Windows so far.

## Roadmap

- Full parsers for Python and JavaScript/TypeScript
- Better project entrypoint detection
- Go, Rust, and Ruby adapters
- cache garbage collection
- clearer dependency conflict diagnostics
- optional configuration while preserving zero setup
