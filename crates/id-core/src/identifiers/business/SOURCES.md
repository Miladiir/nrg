# Normative scope and primary sources

Checked on 2026-08-14.

## German VAT identification number

- Bundeszentralamt für Steuern, *Aufbau der
  Umsatzsteuer-Identifikationsnummer in den EU-Mitgliedstaaten*:
  <https://www.bzst.de/SharedDocs/Downloads/DE/Merkblaetter/ust_idnr_aufbau.pdf?__blob=publicationFile&v=2>
  - Germany uses `DE` followed by nine decimal digits.
- European Commission, *VIES FAQ* and technical information:
  <https://ec.europa.eu/taxation_customs/vies/#/faq>
  <https://ec.europa.eu/taxation_customs/vies/technicalInformation.html>
  - the Commission explicitly says that Member States' VAT-number algorithms
    cannot be disclosed;
  - a VIES/BZSt query, not the offline shape check, establishes current registry
    validity.

The core therefore exposes format validation, `checksum: not_available`,
`lookup: not_performed`, and `assignment: unknown` as separate facts.  It does
not invent a German USt-IdNr. checksum algorithm and deliberately provides no
generator because any format-plausible value could be assigned.

## Legal Entity Identifier

- ISO, *ISO 17442-1:2020 — Financial services — Legal entity identifier (LEI)
  — Part 1: Assignment* (confirmed current in 2026):
  <https://www.iso.org/standard/78829.html>
- Global Legal Entity Identifier Foundation, *Common Data File Formats —
  Questions and Answers*, version 2.4, sections 2.5 through 2.8:
  <https://www.gleif.org/lei-data/access-and-use-lei-data/2022-02-22_cdf_questions_and_answers_v2.4.pdf>
  - exactly 20 uppercase ASCII alphanumeric characters;
  - the final two characters are numeric check digits;
  - checksum calculation follows ISO 17442 / ISO 7064 MOD 97-10;
  - the first four characters are an issuer prefix and characters 5 through 18
    are issuer-assigned entity-specific content;
  - checksum correctness is not evidence about LEI reference data.
- GLEIF, *GLEIF API*:
  <https://www.gleif.org/en/lei-data/gleif-api/>
  <https://api.gleif.org/docs>
  - current assignment and registration state require a Global LEI Index
    lookup; the offline core only constructs the canonical record URL.

The module validates syntax and checksum but deliberately provides no LEI
generator.  Only an accredited LEI Issuer can allocate a production LEI.
