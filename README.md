# runciv

[![license](https://img.shields.io/github/license/hopfenspace/runciv?color=blue)](LICENSE)
[![dependency status](https://deps.rs/repo/github/hopfenspace/runciv/status.svg)](https://deps.rs/repo/github/hopfenspace/runciv)
[![ci status](https://img.shields.io/github/actions/workflow/status/myOmikron/kraken-project/linux.yml?label=CI)](https://github.com/hopfenspace/runciv/actions/workflows/linux.yml)

runciv is a server for [unciv](https://github.com/yairm210/Unciv) 
written in pure rust!

## Building the server

At the time of this writing, there is no precompiled server available,
you have to compile it from source.

### Dependencies

You need to have `cargo` installed. 
The easiest way to retrieve it, is through [rustup](https://rustup.rs/).

On debian-like systems `build-essential` is also required.

After installation of `cargo`, execute:

### Build from source

Install build dependencies:

```bash
cargo install rorm-cli cargo-make
```

Build the project.

```bash
cargo make
```

The resulting binary will be in `target/release/runciv`.

## System configuration

First, create a user and group for the service:

```bash
useradd -r -U runciv -s /bin/bash
```

Copy the service file to `/etc/systemd/system/` and reload systemd:

```bash
cp runciv.service /etc/systemd/system/
systemctl daemon-reload
```

Install the `runciv` binary:

```bash
install -o root target/release/runciv /usr/local/bin/runciv
```

Create a new database & database user:

```bash
su - postgres -c "createuser -P runciv"
su - postgres -c "createdb -O runciv runciv"
```

Apply the migrations:

```bash
rorm-cli migrate --database-config /etc/runciv/config.toml
```

Copy `example.config.toml` to `/etc/runciv/config.toml` and edit the file
to match your desired configuration.

Finally, restart and enable `runciv`:
```bash
systemctl enable runciv
systemctl start runciv
```

## Suggestions & Discussions

If you'd like to discuss something, use our Discussions :)
