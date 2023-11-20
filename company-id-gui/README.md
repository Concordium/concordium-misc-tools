# Company ID GUI Tool

## Building and Running

Install `tauri-cli`:

```bash
cargo install tauri-cli
```

To build or run, first install the dependencies:

```bash
yarn install
```

Then, to build:

```bash
cargo tauri build
```

To build a signed version for windows the `src-tauri/tauri.conf.json` has to be updated with the correct thumbprint following the last part of the procedure outline in the [documentation](https://tauri.app/v1/guides/distribution/sign-windows#c-prepare-variables).

Or, to run:

```bash
cargo tauri dev
```
