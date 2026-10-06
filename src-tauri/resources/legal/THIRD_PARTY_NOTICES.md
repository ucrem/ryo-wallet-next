# Third-party notices

Ryo Wallet Next is an independent application. Its own source is MIT-licensed.
The bundled Ryo executable artifacts and third-party packages retain their
upstream copyrights and license terms; the application's MIT license does not
replace those terms.

- `RYO-LICENSE.txt` and `RYO-ORIGINAL-LICENSE.txt`: unchanged upstream notices
  from the reviewed Ryo 0.6.1.0 source commit. Both executables are unchanged
  official artifacts; `runtime-inventory.json` records their exact hashes,
  targets, source commit and notice provenance.
- `DEPENDENCY_NOTICES.txt`: original available notice files from the Cargo and
  production npm package graphs, grouped by package/version.
- `dependency-inventory.json`: version/license metadata and notice digests for
  those packages, tied to the exact lockfile hashes. Cargo build dependencies
  and packages for other targets are included conservatively.

Packages whose published archives contain no notice file are marked explicitly
in the inventory. Ryo's internal static-library composition is not established
by the executable hashes. These records do not replace review of that upstream
binary composition or platform/system-library redistribution requirements.

Upstream source: https://github.com/ryo-currency/ryo-currency

Ryo names and marks do not imply official endorsement of this application.
