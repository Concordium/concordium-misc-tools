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

To build a signed version for MacOS, you have to add some environment variables to the build command:
* `APPLE_ID` - The apple id used for signing
* `APPLE_PASSWORD` - The password for the apple ID
* `APPLE_TEAM_ID` - ID of concordiums Apple developer team
* `APPLE_SIGNING_IDENTITY` - The Identity of the signing creificate
* `APPLE_CERTIFICATE` - base64 encoding of an .p12 exported version of the signing certificate
* `APPLE_CERTIFICATE_PASSWORD` - The password to the exported version of the signing certificate

Start by running:
```bash
security find-identity -v -p codesigning
```
to find the `APPLE_SIGNING_ID` and the `APPLE_TEAM_ID` in the format:
```bash
1) <APPLE_SIGNING_ID> "Developer ID Application: Concordium Software Aps (<APPLE_TEAM_ID>)"
```
Download the .p12 certificate from bitwarden and run the following command to base64 encode it:
```bash
openssl -in /path/to/.p12-certificate -out base64-certificate.txt
```
A more detailed guide can be found in [the tauri documentation](https://tauri.app/v1/guides/distribution/sign-macos).

To run:

```bash
cargo tauri dev
```
