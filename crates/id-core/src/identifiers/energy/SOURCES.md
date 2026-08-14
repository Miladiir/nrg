# Normative scope and sources

Checked on 2026-08-14.

The reviewed formation rules used by the implementation are also represented
in the machine-readable `data/bdew_identifiers_v1.2.json` catalog. It contains
rule metadata only: no BDEW or DVGW allocation records. The catalog records the
upstream PDF SHA-256
`8864853a19008c82267827436d5393b42dea7e3a44b3c7e4cdeecc8f92379820`
as observed on the checked date; `cargo xtask check-reference-data` validates
the canonical projection and its matching `id-core` constants offline.

## Implemented rules

The implemented MP-ID, NeBe-ID, resource-ID and package-ID syntax and checksum rules come from the
official BDEW application guide:

- BDEW, *Identifikatoren in der Marktkommunikation: Bildungsvorschriften und
  Vergabeprozesse*, version 1.2, 2025-02-07:
  <https://www.bdew.de/media/documents/AWH_Identifikatoren-in-der-Marktkommunikation_Version.1.2.pdf>
  - sections 2.2 and 2.3: 13-digit BDEW/DVGW MP-ID shape, issuer prefixes,
    allocation-mode digit ranges, and Lok-und-Waggon checksum;
  - sections 6.6 and 6.7: CR/SG/SR/TR prefixes, 11-character resource-ID
    shape, and BDEW ASCII checksum;
  - sections 5.2/5.3 and 7.2/7.3: NeBe prefix `F`, package prefix/issuer `P9`,
    11-character shapes, and the BDEW ASCII checksum;
  - sections 8.1 and 8.2: worked checksum procedures and official worked
    examples.
- The current BDEW Redispatch page still links version 1.2:
  <https://www.bdew.de/energie/redispatch-20/>
- The BDEW code-allocation pages confirm that BDEW MP-IDs and resource IDs are
  centrally allocated:
  <https://bdew-codes.de/Codenumbers/BDEWCodes>
  <https://bdew-codes.de/Codenumbers/ResourceId>
- The DVGW S&C portal confirms that DVGW allocates code numbers for market
  participants in the German gas market:
  <https://codevergabe.dvgw-sc.de/>

The BDEW guide publishes `A1137355925` as the worked ASCII-checksum example.
The MP-ID validator test also uses `9979425000005`, an actual BDEW MP-ID
published in the official BDEW guide *Redispatch 2.0: Information zu
Marktrollen, Verantwortlichkeiten und Marktpartner-Identifikationsnummer
(MP-ID)* (2021-05-03):
<https://bdew-codes.de/Content/Files/ResourceId/hinweis-fur-anlagenbetreiber-zur-marktpartner-id-im-redispatch-20.pdf>

The v1.2 formation guide does not publish a designated DVGW test fixture. The
DVGW unit-test value `9801234567895` is therefore explicitly treated as a
derived format/checksum vector, not as an allocated value or an official test
identifier.

## Deliberate non-claims

- These modules do not query BDEW or DVGW registries.
- Successful syntax/checksum validation does not prove allocation.
- Deterministically generated fixtures have no non-collision guarantee against
  centrally allocated identifiers.
- Generated and validated values therefore always report central allocation as
  `Unknown`; there is no `assigned`, `unassigned`, or `non-routable` result in
  this core.
