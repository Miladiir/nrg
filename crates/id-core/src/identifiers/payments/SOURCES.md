# Normative scope and primary sources

Checked on 2026-08-14.

## IBAN and MOD 97-10

- SWIFT, the ISO 13616 Registration Authority, *IBAN Registry*, release 102
  (June 2026): <https://www.swift.com/swift-resource/9606/download>
  - general IBAN structure and ISO/IEC 7064 MOD 97-10;
  - Germany: `DE2!n8!n10!n`, total length 22, eight-digit bank identifier,
    ten-digit account identifier;
  - registry test vector `DE89370400440532013000` and its print form.
- Deutsche Bundesbank, *IBAN-Regeln*:
  <https://www.bundesbank.de/de/aufgaben/unbarer-zahlungsverkehr/serviceangebot/iban-regeln>
  - German BBAN composition and the warning that institution-specific IBAN
    derivation rules can apply.
- Deutsche Bundesbank, versioned bank-code file embedded by the surrounding
  `reference_data` module:
  <https://www.bundesbank.de/resource/blob/926192/b27b518a016ea7ca7af321eb7289fcf4/472B63F073F071307366337C94F8C870/blz-aktuell-csv-data.csv>
  - snapshot validity 2026-06-08 through 2026-09-06;
  - SHA-256 `3a5484358b326d6d9ed8bea003601a7d79abbc100cdb81fd0d30e0c8a21898a6`.

The core checks syntax, country-specific shape, MOD-97 and (when supplied) the
snapshot's bank-code presence. It never claims that an account exists. A
`synthetic_non_routable` result is relative to the exact supplied snapshot and
must be recalculated after a reference-data update.

## BIC

- SWIFT, as ISO 9362 Registration Authority, *ISO 9362:2014 — BIC
  Implementation*: <https://www.swift.com/swift-resource/14256/download?language=en>
  - 4 alphanumeric business-party-prefix characters, 2 alphabetic country
    characters, 2 alphanumeric party-suffix/location characters, and an
    optional 3-character branch identifier;
  - software is advised to accept the ISO alphanumeric prefix even though
    SWIFT itself continues to issue alphabetic prefixes;
  - `0` in position 8 is the Test & Training address convention, and these
    addresses are not published in the normal BIC directory.
- ISO, *ISO 3166 — Country Codes*:
  <https://www.iso.org/iso-3166-country-codes.html>
  - officially assigned alpha-2 country-code set; the embedded set was checked
    on 2026-08-14 and deliberately rejects user-assigned elements such as `ZZ`.

The generator therefore calls its output a T&T **pattern**, not a registered
SWIFT test BIC. Country-code syntax and the embedded assigned ISO 3166 set are
checked, while current SWIFT registration still requires external reference
data.

## German SEPA Creditor Identifier

- Deutsche Bundesbank, *Häufig gestellte Fragen zu der
  Gläubiger-Identifikationsnummer*:
  <https://www.bundesbank.de/dynamic/action/de/aufgaben/unbarer-zahlungsverkehr/serviceangebot/sepa/glaeubiger-identifikationsnummer/642684/haeufig-gestellte-fragen-zu-der-glaeubiger-identifikationsnummer?contentId=640170&firstLetter=W>
  - German 18-character structure;
  - official test fixture `DE98ZZZ09999999999`.
- European Payments Council, *SEPA Core Direct Debit Inter-bank
  Implementation Guidelines*, creditor-identifier format rules:
  <https://www.europeanpaymentscouncil.eu/sites/default/files/KB/files/EPC114-06%20SDD%20Core%20Interbank%20IG%20V9.0%20Approved.pdf>
  - MOD 97-10 calculation and exclusion of positions 5 through 7 (Creditor
    Business Code) from the checksum.

## Mandate reference, End-to-End ID, and common SEPA text rules

- European Payments Council, *SEPA Direct Debit Core e-Mandate Service
  Implementation Guidelines 2025*, `Mandate Identification` / `Max35Text`:
  <https://www.europeanpaymentscouncil.eu/sites/default/files/kb/file/2025-10/EPC002-09%20SDD%20Core%20e-Mandate%20Service%20IG%202025%20V1.0.pdf>
- European Payments Council, *SEPA Credit Transfer Inter-PSP Implementation
  Guidelines 2025*, `EndToEndId` / `Max35Text` and `NOTPROVIDED`:
  <https://www.europeanpaymentscouncil.eu/sites/default/files/kb/file/2025-10/EPC115-06%20SCT%20Inter-PSP%20IG%202025%20V1.0.pdf>
- European Payments Council, *Clarification Paper on the Use of Slashes in
  References, Identifications and Identifiers*, version 2.0:
  <https://www.europeanpaymentscouncil.eu/document-library/guidance-documents/clarification-paper-use-slashes-references-identifications-and>
  - no leading or trailing `/` and no `//` in SEPA references/identifiers.

Generators deliberately use only uppercase ASCII letters, digits, and `-`,
even though validation accepts the interoperable basic Latin SEPA subset.

## RF Creditor Reference

- ISO, *ISO 11649:2009 — Structured creditor reference to remittance
  information* (confirmed current in 2025):
  <https://www.iso.org/standard/50649.html>
- Finance Finland, *Structure of the RF Creditor Reference (ISO 11649)*:
  <https://www.finanssiala.fi/wp-content/uploads/2024/04/structure-of-the-rf-creditor-reference-iso-11649.pdf>
  - `RF`, two IBAN-style check digits, and at most 21 freely formed
    alphanumeric body characters; four-character print grouping.
- European Payments Council, *Quick Response Code — Guidelines to Enable the
  Data Capture for the Initiation of an SCT*, version 3.1:
  <https://www.europeanpaymentscouncil.eu/sites/default/files/kb/file/2024-03/EPC069-12%20v3.1%20Quick%20Response%20Code%20-%20Guidelines%20to%20Enable%20the%20Data%20Capture%20for%20the%20Initiation%20of%20an%20SCT.pdf>
  - published RF vector `RF18539007547034`.

## UETR

- SWIFT, *Certified Application — Payments — Label Criteria*, UETR format:
  <https://www.swift.com/swift-resource/125871/download?language=en>
  - a UETR is a UUID compliant with version 4 of RFC 4122;
  - its exact representation is
    `xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx`, using lower-case hexadecimal only,
    where `y` is `8`, `9`, `a`, or `b`.
- SWIFT, *What is a Unique End-to-end Transaction Reference (UETR)?*:
  <https://www.swift.com/payments/what-unique-end-end-transaction-reference-uetr>
  - a UETR is transported unchanged through the payment chain.

The deterministic UETR generator is limited to reproducible test fixtures.  It
does not provide cryptographic randomness or a global collision guarantee and
marks every result as not production-usable.

## Deliberate non-claims

- Syntax and checksum validity do not prove assignment, reachability, bank or
  account existence, or production usability.
- Deterministic generators are test-fixture functions, not allocation
  authorities and not cryptographic random-number generators.
- No SWIFT BIC or international IBAN directory is embedded by these payment
  modules. The embedded ISO 3166 alpha-2 set must be reviewed when ISO publishes
  a change.
