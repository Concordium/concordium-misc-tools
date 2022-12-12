# The ID demo test tool

This is a companion frontend application to go with the
[id-verifier](../id-verifier) server. It connects the `id-verifier` to the
browser extension wallet for testing.

This is a Rust project, but its build artifact is a Javascript + Wasm single
page application. It uses [yew](https://yew.rs/).

## Build

The easiest way to build is to use the [`trunk`](https://trunkrs.dev/) tool.
Follow the instructions for its installation.

The page can be built by using
```
ID_TESTER_BASE_URL=http://localhost:8100 trunk build --release
```

Inf the `ID_TESTER_BASE_URL` variable is not set it defaults to
`http://localhost:8100`. It has to be the address where the `id-verifier` is
reachable.

This produces all the artifacts in a `dist` directory, including a `index.html`
file. The page can be served by serving that directory.

## Development

`trunk` supports a development server which will start a local server and reload
the page on changes. It can be started as

```
ID_TESTER_BASE_URL=http://localhost:8100 trunk serve
```
