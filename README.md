![License](http://img.shields.io/badge/license-MIT-lightgrey.svg)
[![Build Status](https://travis-ci.org/phsym/rust-sctp.svg)](https://travis-ci.org/phsym/rust-sctp)
[![Crates.io](https://img.shields.io/crates/v/rust-sctp.svg)](https://crates.io/crates/rust-sctp)

# rust-sctp

[Documentation](http://phsym.github.io/rust-sctp)

SCTP networking library for Rust

# How to build

`rust-sctp` relies on the [sctp-sys](https://crates.io/crates/sctp-sys) crate. Please have a look at [sctp-sys: How to build](https://github.com/phsym/sctp-sys#how-to-build).

> **WARNING:** Windows support is currently broken and unmaintained as SctpDrv is not working on modern windows platforms