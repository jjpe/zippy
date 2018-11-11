# Zippy
Zippy is a simple CLI-based utility that provides basic zip/unzip functionality.
It is intended to be a simple but useful component during the packaging step of
complex software.

## Installation
Installing the Zippy is as easy as `cargo install zippy`.

## Usage
Zippy has some sub-commands that help it perform its tasks:
* `zippy zip    --input FILE+     --output ZIP_FILE`
* `zippy unzip  --input ZIP_FILE  --output DESTINATION_DIR`
* `zippy help`
