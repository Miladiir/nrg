# Register identifier sources and scope

Checked on 2026-08-14.

## Marktstammdatenregister numbers

- Bundesnetzagentur, *MaStR - Nummernkonzept*, May 2019:
  <https://www.marktstammdatenregister.de/MaStRHilfe/files/regHilfen/MaStR-Nummernkonzept.pdf>
- Bundesnetzagentur, MaStR web-service documentation, version 26.1.177:
  <https://www.marktstammdatenregister.de/MaStRHilfe/subpages/webdienst.html>

The number concept is the primary source for the three-letter prefixes, the
optional market-role suffix matrix and the EAN-style check digit. The current
web-service documentation was used as a cross-check for prefixes exposed by the
production interface. Syntax and checksum validation do not query the MaStR and
therefore never establish allocation. The legacy `SME` and `GME` migrated-unit
prefixes remain parseable because the number concept explicitly defines them.

The reviewed vocabulary and role matrix are embedded as the canonical,
machine-readable `data/mastr_prefixes_2019-05.json` catalog. It records the
source-PDF SHA-256
`e2154964b260c5d53274c065ae873114eb048d421bbba65a702f0ffbc56ba01c`
as observed on 2026-08-14, includes no allocation records and is checked
against the runtime enum and role matrix by `id-core` tests and the reference
data xtask.

Generated MaStR values are labelled synthetic, not production-usable, with
unknown allocation status and no collision guarantee. A checksum-valid value
can collide with a centrally allocated identifier.

## Energy Identification Code (EIC)

- ENTSO-E, *The Energy Identification Coding Scheme (EIC), Reference Manual*,
  version 5.4, 2021-09-15:
  <https://eepublicdownloads.entsoe.eu/clean-documents/EDI/Library/EIC_Reference_Manual_Release_5_4.pdf>
- ENTSO-E, *EIC data exchange implementation guide*, version 1.1:
  <https://eepublicdownloads.entsoe.eu/clean-documents/EDI/Library/cim_based/EIC_Data_Exchange_IG_v1.1.pdf>
- ENTSO-E EIC registry and documentation landing page:
  <https://www.entsoe.eu/data/energy-identification-codes-eic/>
- ENTSO-E public allocated-code bulk XML:
  <https://eepublicdownloads.blob.core.windows.net/cio-lio/xml/allocated-eic-codes.xml>

The parser implements the published 2-character LIO code, 1-character object
type, 12-character local identifier and final check character. The validator
uses the published MOD-37-style check-character algorithm solely to check a
provided value. The implementation guide expressly limits use of that
algorithm for allocation to authorised LIOs, so this module intentionally has
no EIC generator. A valid check character does not prove a registry entry or an
active allocation.

NRG embeds a deterministic projection of the public bulk document rather than
querying the network during a request. The checked snapshot was created at
`2026-08-13T01:15:13Z` and retains all published `A05` active and `A03`
inactive records. Directory hits and misses are always qualified by that
snapshot. In particular, absence is not treated as proof of non-allocation,
because local LIO registries and later changes can differ.
