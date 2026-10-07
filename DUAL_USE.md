# Dual-Use Notice and Responsible Use

> **§5 is provisional.** A declaration covering this software is being prepared for
> France's national cybersecurity agency (ANSSI), under décret n° 2007-663. That filing
> adds to §5 rather than changing it: no export classification has been obtained, and an
> ANSSI declaration is not one. The rest of this document is not affected. This note goes
> when the declaration is filed.

---

## 1. What this software is

ØMYSTIK is a peer-to-peer mesh in which:

- nodes discover each other and have distinctive roles;
- computation over data can be performed **while the data stays encrypted**, using
  fully homomorphic encryption (FHE) and threshold key management;
- content is encrypted (XChaCha20Poly1305) and addressed by content identifier;
- content's metadata is hashed into Merkle structures so it can be queried without being published
  in the clear;

Those properties exist to protect the people whose data crosses the mesh. The same
properties make the system attractive to actors whose purposes are not legitimate. That
tension is inherent to strong cryptography and to resilient networking; it cannot be
engineered away, only acknowledged and managed.

## 2. Intended uses

*These are the uses the architecture is aimed at. None of them is a claim that this release
is fit for them. [§6](#6-state-of-development) states what has actually been built and
tested.*

ØMYSTIK is intended for lawful use by organizations that need confidentiality and
resilience in constrained or contested environments. Representative examples:


**Civil**
- Disaster response and civil protection where fixed infrastructure has failed
- Medical and humanitarian coordination involving patient or beneficiary data
- Judicial and legal workflows with confidentiality obligations
- Scientific collaboration on sensitive or pre-publication data
- Industrial and economic-intelligence protection

**Defense and security**
- Command, control and situational awareness for units operating with degraded links
- Coalition data sharing where participants must collaborate without pooling raw data
- Logistics and convoy coordination (see the OSATCON reference application)

## 3. Uses this project rejects

The maintainers do not support, and ask that you do not use this software for:

- **Mass or indiscriminate surveillance**, including bulk collection or analysis of
  communications or movements of people who are not the subject of a lawful,
  particularized authorization.
- **Targeting of civilians**, or any use forming part of an operation that violates
  international humanitarian law.
- **Suppression of human rights.** Surveilling, locating, or building profiles of
  journalists, lawyers, human-rights defenders, political dissidents, minority groups,
  or their families.
- **Autonomous use of force.** ØMYSTIK carries data and computes over it. It must not be
  wired into a weapon-release or engagement decision that no accountable human authorizes.
- **Concealing unlawful activity**, including using the mesh's confidentiality properties
  to evade lawful oversight of your own conduct.
- **Circumventing sanctions or export controls** (see [§5](#5-export-control-and-sanctions)).

This section is a statement of the project's position and a condition of the
maintainers' support. It is **not** an additional restriction on the software license:
the BSD-3-Clause-Clear license in [LICENSING.md](LICENSING.md) governs your legal rights
and grants freedoms this section does not revoke. What the maintainers control is
whether they will assist, accept contributions from, or collaborate with a given
deployment, and that is withheld from uses listed above.

## 4. Obligations of the deployer

If you deploy ØMYSTIK you take on responsibilities the software cannot discharge for you:

1. **Legal basis.** Establish that your processing has a lawful basis in every
   jurisdiction where a node runs, and that operating an encrypted mesh is itself lawful
   there. Some jurisdictions restrict cryptography, mesh networking, or specific radio
   bearers.
2. **Authorization and accountability.** Maintain records of who authorized each
   deployment and each class of data processed. The mesh is deliberately
   privacy-preserving; it will not produce an audit trail of content on your behalf.
3. **Key custody.** Threshold FHE distributes trust across parties. If one operator
   controls enough parties to reconstruct, the threshold guarantee is nominal. Document
   your party distribution and who runs each one.
4. **Downstream notice.** If you redistribute or integrate ØMYSTIK, carry this document
   forward.

## 5. Export control and sanctions

ØMYSTIK incorporates strong cryptography, including FHE, and is documented for defense
applications. Cryptographic software and defense-related technology are export-controlled
in many jurisdictions.

**The maintainers have not obtained an export classification for this software and make
no representation about its export status.** Nothing here is legal advice.

Regimes that may apply, depending on where you are and where the software goes:

- **EU.** Regulation (EU) 2021/821 (dual-use recast). Cryptography falls under
  Category 5 Part 2; some defense-adjacent items fall under national military lists.
- **France.** Two separate obligations. The *Agence nationale de la sécurité des systèmes
  d'information* (ANSSI) regime covers supplying, transferring and importing a means of
  cryptology, and for most functions is satisfied by declaration. Taking the software
  outside the EU is a different question, handled by the *Service des biens à double usage*
  (SBDU) under the dual-use regulation. A declaration made to ANSSI is not an export
  classification and authorizes no export.
- **United States.** The Export Administration Regulations (EAR), Category 5 Part 2 for
  encryption; ITAR if a deployment makes the software a defense article.

These regimes share common technical categories because they all implement the Wassenaar Arrangement, the multilateral basis behind most dual-use export control.

Before you export, re-export, or make this software available across a border, or provide
it to a sanctioned party or embargoed destination, obtain your own classification and
authorization. If you contribute code, do not contribute anything you are not permitted to
publish.

## 6. State of development

Three things are worth stating plainly for anyone assessing the system, because the
distance between what ØMYSTIK is designed for and what it has been shown to do is large.

**Only ordinary IP networks have been exercised.** Every test of this release ran over
Wi-Fi and conventional internet links, from machines on a local network out to compute
instances on Google Cloud Platform. Nothing has been developed for the bearers a
deployment in the field would actually use: LoRa, satellite links, tactical radio, or any
path that is low-bandwidth, high-latency, or intermittent by nature. The mesh has never run
over one, so how it behaves there is unknown rather than degraded. Read the resilience
properties described anywhere in this repository as design intent that remains to be
demonstrated.

**Decentralized AI is not implemented.** Federated learning, deep FHE inference, and
shared decentralized compute for training appear in the project's roadmap and in
earlier project descriptions. They are **not in this codebase**. Do not represent
ØMYSTIK as providing them today.

**This is pre-production software.** See [SECURITY.md](SECURITY.md) for the current
security posture, which includes an `insecure` feature enabled in the default build and
test configurations that derive keys from short deterministic seeds. It has not undergone
independent security audit or cryptographic review. Do not protect real lives or real
secrets with it in its present state.

Taken together: the defense uses in [§2](#2-intended-uses) describe where the architecture
points, not a capability on offer. Substantial development separates this release from any
of them, and the project is best understood at this stage as experimental research code
published so that the approach can be examined.

## 7. Reporting misuse

If you become aware of a deployment of ØMYSTIK used for any purpose in [§3](#3-uses-this-project-rejects), contact the
maintainer (see [SECURITY.md](SECURITY.md)). Reports concerning an active threat to
someone's safety should go to the appropriate authority first; the maintainers cannot
intervene operationally.

---

*This document covers responsible use. For vulnerability reporting see
[SECURITY.md](SECURITY.md); for license terms and the Zama commercial-use obligation see
[LICENSING.md](LICENSING.md).*
