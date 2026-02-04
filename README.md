This is a fork of the original [cp_sat](https://github.com/KardinalAI/cp_sat) repository. As of writing, the original repo supports OR-TOOLS v9.0 where as this fork is updated to support v9.14

# Google CP-SAT solver Rust bindings

Rust bindings to the Google CP-SAT constraint programming solver.

## Prerequisites
- A C++ compiler (e.g. clang)
- [ortools](https://github.com/google/or-tools) v9.14 and its dependencies

The crate will search for the ortools library in default locations for your platform. If you installed ortools in a
custom location, set the `OR_TOOLS_LIB_DIR` and `OR_TOOLS_INCLUDE_DIR` environment variables.

## Limitations

If protobuf is not installed in a default location for your system, you will need to manually set the correct flag for
`rustc` to find it.

Only Linux and macOS are supported.
