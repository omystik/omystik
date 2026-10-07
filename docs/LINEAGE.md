# Lineage

Where ØMYSTIK came from, and which parts of it are inherited rather than new.

ØMYSTIK is not a single project that grew in one line. It draws on three earlier projects by
the same author, developed independently of one another, each addressing a different problem.
Two of them are implemented in the codebase today. The third is the origin of work that
remains on the roadmap, and this document is explicit about which is which.

For what the system does now, read [ARCHITECTURE.md](ARCHITECTURE.md). For what is and is not
implemented, see the "What works, and what doesn't" section of the
[README](../README.md).

---

## Three lineages

```
                              ØMYSTIK
                                 ▲
                                 │
          ┌──────────────────────┼──────────────────────┐
          │                      │                      │
    MYSTIK>p2p              FHE.Chess               FHE.IRM
          │                      │                      │
  persistent encrypted    deep learning in FHE    blockchain in FHE
     p2p storage          Concrete ML, TFHE       fhEVM, KMS, TFHE
       MID / CID          quantized CNN           encrypted-data operations
   content discovery      homomorphic inference   rights management
```

None of the three derives from another. They converge in ØMYSTIK rather than succeed one
another.

---

## MYSTIK>p2p

*November 2024 to March 2025. Implemented in ØMYSTIK today.*

A persistent encrypted peer-to-peer storage network. Files are encrypted before they touch
disk and addressed by a content identifier derived from the ciphertext. Metadata is hashed
into a Merkle tree whose root serves as a lookup identifier, so a node can advertise what it
holds without publishing what it is.

This is the most directly inherited of the three. The storage model is Mesh A today: the same
CID and MID construction, the same two Merkle layers, the same principle that the network
never holds the decryption key.

The original project description is kept in this repository at
[README_mystik-p2p.md](../README_mystik-p2p.md), because it remains the clearest account of
the storage model.

---

## FHE.Chess

*2023. Not implemented in ØMYSTIK. Origin of a roadmap item.*

Repository: <https://github.com/vrona/FHE.Chess>

A chess application whose AI opponent infers over a board it cannot read. Move prediction is
carried by two convolutional neural networks, Source and Target, trained in PyTorch, then
retrained quantization-aware with Brevitas at 4-bit weights and activations, then compiled
with Zama's Concrete ML so that inference executes homomorphically. The application exposes
three modes, `clear`, `simfhe` and `deepfhe`, the last performing the inference while the
data stays encrypted. It was produced in answer to a Zama bounty.

**ØMYSTIK does not contain this capability.** Deep FHE inference and federated learning are
roadmap items, not code. FHE.Chess is where that work was actually done, in a different
shape and on a different problem, and it is cited here as the antecedent of an intended
direction rather than as a component of the current system.

It is also the only one of the three that has always been public, so its history can be
inspected independently.

---

## FHE.IRM

*Spring 2024. Partly implemented in ØMYSTIK.*

A decentralized application managing access rights and updates to confidential documents on
Ethereum, built on Zama's fhEVM, including its key management and TFHE. Where FHE.Chess
applied homomorphic encryption to model inference, FHE.IRM applied it to rights management:
operations over encrypted data, encrypted metadata, and authorization workflows that never
expose the underlying document.

What carries into ØMYSTIK is the encrypted-computation model and the use of threshold key
management, now running over a peer-to-peer mesh instead of a blockchain. The on-chain
rights-management layer does not carry over; ØMYSTIK has no blockchain component.

---

## What is inherited and what is not

| From | Inherited | Status in ØMYSTIK |
|---|---|---|
| MYSTIK>p2p | Encrypted storage, CID/MID addressing, content discovery | Implemented, Mesh A |
| FHE.IRM | Computation over encrypted data, threshold key custody | Implemented, Mesh B and Mesh C |
| FHE.IRM | On-chain rights management | Not carried over |
| FHE.Chess | Deep learning under FHE, quantized model inference | Not implemented, roadmap |

---

## External context

On 2 June 2025, NATO's Defence Innovation Accelerator for the North Atlantic (DIANA)
published its 2026 Advanced Communication Technologies challenge. That challenge
independently identifies several technical areas overlapping the direction of ØMYSTIK:
resilient and decentralised communications, packet-switched mesh networks, edge computing,
homomorphic encryption, and technologies intended for both civilian and military
environments.

It is cited here as external context on the problem space, nothing more. It does not imply
that NATO or DIANA has evaluated, validated, endorsed or approved ØMYSTIK, and no
application, submission or relationship of any kind is claimed.

---

## Third-party provenance

The cryptography in all three projects comes from [Zama](https://www.zama.ai/): Concrete ML
in FHE.Chess, fhEVM in FHE.IRM, TFHE-rs and the KMS in ØMYSTIK. The KMS is vendored into
this repository and modified, which is documented in [kms/UPSTREAM.md](../kms/UPSTREAM.md).

Licence terms, and the Zama commercial-use obligation that applies to anyone building on
this, are in [LICENSING.md](../LICENSING.md).


