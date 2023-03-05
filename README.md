# runciv

[![license](https://img.shields.io/github/license/hopfenspace/runciv?color=blue)](LICENSE)
[![dependency status](https://deps.rs/repo/github/hopfenspace/runciv/status.svg)](https://deps.rs/repo/github/hopfenspace/runciv)
[![ci status](https://img.shields.io/github/actions/workflow/status/myOmikron/kraken-project/linux.yml?label=CI)](https://github.com/hopfenspace/runciv/actions/workflows/linux.yml)

runciv is a server for [unciv](https://github.com/yairm210/Unciv) 
written in pure rust!

## Building the server

You need to have `cargo` installed. 
The easiest way to retrieve it, is through [rustup](https://rustup.rs/).

After installation of `cargo`, execute:

```bash
cargo build -r -p runciv
```

The resulting binary will be in `target/release/runciv`.
