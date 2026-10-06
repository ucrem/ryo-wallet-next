# Third-party notices

Ryo Wallet Next is an independent application. Its own source is MIT-licensed.
The bundled Ryo executable artifacts and third-party packages retain their
upstream copyrights and license terms; the application's MIT license does not
replace those terms.

- `RYO-LICENSE.txt` and `RYO-ORIGINAL-LICENSE.txt`: unchanged upstream notices
  from the reviewed Ryo 0.6.1.0 source commit. Both executables are unchanged
  official artifacts; `runtime-inventory.json` records their exact hashes,
  targets, source commit and notice provenance.
- `DEPENDENCY_NOTICES.txt`: available published notice files and reviewed
  upstream supplements from the Cargo and production npm package graphs,
  grouped by package/version. `AUTHORS` files are retained where supplied.
- `dependency-inventory.json`: version/license metadata and notice digests for
  those packages, tied to the exact lockfile hashes. Cargo build dependencies
  and packages for other targets are included conservatively.
- `supplemental-inventory.json` and `supplemental/`: notices at the source
  commits recorded by published crates when package archives omit them.
  Selectors' published source declares MPL 2.0; the full SPDX standard text
  and source licensing header are included. Its unmodified source is available
  at https://static.crates.io/crates/selectors/selectors-0.36.1.crate.
- `ryo-source-materials.json` and `ryo-source-notices/`: additional notices
  from the reviewed Ryo source tree. This is a conservative source-material
  collection, not a verified bill of materials for the official binaries.

Four packages still have no collected notice file: two Windows GNU import
packages and an Android verifier, which are absent from the selected dependency
graphs of all supported desktop targets; and production npm
`react-remove-scroll-bar 2.3.8`, whose recorded source commit/tag could not be
resolved publicly. Its public source currently declares version 2.3.7, so it
is not represented as publication-matched evidence for 2.3.8.
The remaining gaps are recorded explicitly in the inventory.
Ryo's internal static-library composition is not established
by the executable hashes. These records do not replace review of that upstream
binary composition or platform/system-library redistribution requirements.

Upstream source: https://github.com/ryo-currency/ryo-currency

Ryo names and marks do not imply official endorsement of this application.
