# `p2p-bootstrap`

A minimal [rust-libp2p](https://github.com/libp2p/rust-libp2p) implementation for bootstrapping [libp2p](https://libp2p.io/) compatible services.

## Installing

In order to install this as a binary, one must already install [Rust](https://www.rust-lang.org/tools/install)

```bash
cargo install p2p-bootstrap --git https://github.com/bamboolabs-foundation/p2p-bootstrap
```

## How to run

One can simply find out how with:

```bash
> p2p-bootstrap --help
p2p-bootstrap 0.1.1
Aditya Kresna <aditya.kresna@outlook.co.id>

USAGE:
    p2p-bootstrap [OPTIONS]

OPTIONS:
    -h, --help                       Print help information
    -j, --join-ipfs                  Join global IPFS network
    -p, --port <PORT>                TCP & UDP Port [default: 4011]
    -s, --secret-key <SECRET_KEY>    Ed25519 Secret Key Bytes in hexadecimal [default: {RANDOM}]
    -V, --version                    Print version information
```
