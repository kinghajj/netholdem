# Build Guide

## Prerequisites

* [A Rust toolchain](https://rustup.rs/). Use the latest stable release for best
  results.
* [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/).
* [npm](https://www.npmjs.com/get-npm). We're using v12.
* [rollup](https://www.npmjs.com/package/rollup). v2.6+.
* [uglify](https://www.npmjs.com/package/uglify-es). v3.3+.

Additionally, you'll need to install development libraries for SSL.

The Dockerfile under `$PROJECT_ROOT/containers/builder` shows all of the steps
needed to setup a build environment for Debian-based systems.

## Building

Once your build environment is setup, you should be able to run

```sh
$ make
```

To make a Debian package, run

```sh
$ make pkg
```
