# Metering identifier sources and scope

Checked on 2026-08-14.

## OBIS

- DLMS User Association, *Blue Book Part 1*, DLMS UA 1000-1 Part 1,
  Edition 17, 2025-02-28 (public excerpt):
  <https://www.dlms.com/wp-content/uploads/2025/06/Excerpts-DLMS-Blue-Book-Ed-17-part-1-V1.0.pdf>
- Bundesnetzagentur, Mitteilung Nr. 54, published 2025-10-01, effective
  2026-04-01; links the binding *Codeliste der OBIS-Kennzahlen und Medien 2.5c*:
  <https://www.bundesnetzagentur.de/DE/Beschlusskammern/BK06/BK6_83_Zug_Mess/835_mitteilungen_datenformate/Mitteilung_54/Mitteilung_Nr_54.html>
- BDEW MaKo, *Codeliste der OBIS-Kennzahlen und Medien 2.5c*:
  <https://www.bdew-mako.de/pdf/Codeliste-OBIS-Kennzahlen_Medien_2_5c_20251001.pdf>

The parser implements the six value groups A through F and their public ranges.
It accepts full display, reduced display and complete six-byte logical-name
notation. The embedded market catalog contains only a small set of common
electricity patterns that are explicit in BNetzA/BDEW 2.5c. It is labelled as
non-exhaustive: a lookup miss is not evidence that an OBIS code is invalid or
unstandardised. Manufacturer-, utility-, consortium- and country-specific
ranges make such an inference especially unsafe.

## DIN 43849 device identifier

- DIN Media / DKE, *DIN 43849:2024-05 - Messeinrichtungen und -systeme,
  sowie Zusatzeinrichtungen und Steuergeräte - Herstellerübergreifende
  Identifikationsnummer*:
  <https://www.dinmedia.de/de/norm/din-43849/377951610>
- OMS Group, *Open Metering System Specification Vol. 2 - Primary
  Communication*, Issue 5.0.1, 2023-12, section 3.2:
  <https://oms-group.org/wp-content/uploads/2024/10/OMS-Spec_Vol2_Primary_v501_01.pdf>
- DLMS User Association, official FLAG ID structure and directory:
  <https://www.dlms.com/flag-id/>

DIN 43849 is not freely available in full. The validator therefore stops at
the publicly corroborated field structure: 14 electronic characters made from
one OBIS category, a three-uppercase-letter manufacturer ID, a two-digit
fabrication block and an eight-digit fabrication number. The category mapping
is a conservative public subset. Unknown uppercase alphanumeric categories are
reported as unclassified rather than falsely rejected or assigned a meaning.
No checksum is claimed, no FLAG-directory lookup is performed and no generator
is provided: a fabricated manufacturer/serial combination cannot be guaranteed
unassigned.
