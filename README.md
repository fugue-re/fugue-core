<p align="center">
  <img alt="Fugue logo" src="https://raw.githubusercontent.com/fugue-re/fugue-core/master/data/fugue-logo-border-t.png" width="20%">
</p>

# Fugue Binary Analysis Framework


[![DOI](https://zenodo.org/badge/386728913.svg)](https://zenodo.org/badge/latestdoi/386728913)


Fugue is a binary analysis framework in the spirit of [B2R2] and [BAP], with
a focus on providing reusable components to rapidly prototype new binary
analysis tools and techniques.

Fugue is built around a core collection of crates, i.e., `fugue-core`. These
crates provide a number of fundamental capabilities:

- Data structures and types:
  - Architecture definitions (`fugue-arch`).
  - Bit vectors (`fugue-bv`).
  - Floating point numbers (`fugue-fp`).
  - Endian-aware conversion to and from various primitive types
    (`fugue-bytes`).

- Program representations and abstractions:
  - A knowledge database to represent program binaries that can be populated
    using third-party tools (`fugue-db`).
  - Disassembly and lifting to intermediate representations (`fugue-ir`).

## Prerequisites

```
git submodule init
git submodule update --recursive
```

## Build

```
cargo build
```

[BAP]: https://github.com/BinaryAnalysisPlatform/bap/
[B2R2]: https://github.com/B2R2-org/B2R2
