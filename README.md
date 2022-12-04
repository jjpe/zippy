# Zippy

[![](https://img.shields.io/crates/v/zippy?label=zippy)](https://crates.io/crates/zippy)
![](https://img.shields.io/badge/rustc-1.51+-darkcyan.svg)
![](https://img.shields.io/crates/l/zippy)

Zippy is a simple CLI-based utility that provides basic zip/unzip functionality.
It is intended to be a simple but useful component during the packaging step of
complex software.

## Installation
Installing the Zippy is as easy as `cargo install zippy`.

## Usage
Zippy has some sub-commands that help it perform its tasks:
* `zippy zip    --input FILE+
                --output ZIP_FILE
                --method [bzip2, deflate, store, zstd]
                --level <LEVEL>`
  where the `--method` and `--level` args are optional.
* `zippy unzip  --input ZIP_FILE
                --output DESTINATION_DIR`
* `zippy help`
