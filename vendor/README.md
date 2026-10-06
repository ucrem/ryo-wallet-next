# GLib iterator backport

`glib/` is the unmodified crates.io `glib 0.18.5` archive, except for the two
pointer-mutability changes in `src/variant_iter.rs` from the official fix
[gtk-rs-core #1343](https://github.com/gtk-rs/gtk-rs-core/pull/1343), commit
`b5a4071e439bef2b5eea76c3aa25e5ae84839e34`.

The archive digest, original/patched file digests and exact upstream diff are
recorded in `glib/RYO_PATCH_PROVENANCE.json`. The original license is preserved.
GTK/WebKit's current dependency graph requires GLib 0.18; substituting GLib 0.20
would not patch the selected 0.18 dependency. The local Cargo patch keeps the
existing API and includes the official correction for RUSTSEC-2024-0429.

The Linux CI regression exercises all affected iterator methods with the GLib
dependency optimized. Packaging checks the patched file against the reviewed
notice inventory. Version-only vulnerability tools may still report 0.18.5;
this is a source backport, not a claim that that registry version is safe.

Remove this directory, the Cargo patch, its test profile and the direct Linux
test dependency together when the GTK dependency graph accepts a fixed upstream
GLib release. Never publish this patched crate as the upstream package.
