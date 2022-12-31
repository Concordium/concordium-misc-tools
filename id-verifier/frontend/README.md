## Frontend for testing ID 2.0 proofs using wallets

### Build

This is a typescript project. You need `yarn` to build it. To build run

```
yarn install
yarn build
```

This will produce `index.html` and `index.js` in a `dist` directory. That is the artifact that needs to be served.

By default the frontend expects to be served from the same URL as the verifier.
To change the verifier URL modify the `getVerifierURL` function in `index.tsx`.

### Development

Use `yarn watch` to automatically rebuild upon changes.
Use `yarn lint` to check formatting and common issues. Use `yarn lint-and-fix` to automatically fix a number of issues (e.g., formatting).
Use `yarn serve` to serve the pages locally. When doing this make sure to change `getVerifierURL` to point to the URL of the verifier.