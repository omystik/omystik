# Contributing to ØMYSTIK

**ØMYSTIK is not yet open to unsolicited pull requests.**

The project is pre-production, unaudited, and maintained by one person. Until the items in
[SECURITY.md](SECURITY.md#known-limitations) are closed, code arriving faster than it can be
reviewed is a risk rather than a help. That is a statement about the project's capacity, not
about the quality of anyone's work.

This will change. When it does, this document changes with it.

---

## What is welcome now

**Bug reports.** Something that does not behave as the documentation says, or does not build
from a clean clone. These are the most useful thing you can send.

**Questions about how it works.** If the documentation left you guessing, that is a defect
in the documentation. Say where you got lost.

**Proposals.** An idea, a design, a change you would like to make. Open an issue and
describe it before writing code, and we can agree the scope first. If it fits, you will be
invited to send the work as a pull request.

**Security defects go somewhere else.** Report them privately, following
[SECURITY.md](SECURITY.md). Never open a public issue for a security defect, and never send
one as a pull request: a public fix for a live vulnerability discloses that vulnerability to
everyone who is watching.

## What to expect

One person maintains this alongside other work. Expect a reply in days rather than hours,
and sometimes longer. If an issue goes quiet for a fortnight, a polite nudge is reasonable
and not unwelcome.

Proposals that fit the project's direction may still be declined on timing. The roadmap is
in [the README](README.md#what-works-and-what-doesnt), and the honest summary is that the
confidential-compute path works while the decentralised-AI layer above it has not been
started.

## Before you open an issue

Two things make an issue immediately actionable:

- **Which part.** The crate, the node type, or the protocol. The three meshes behave very
  differently, and "the mesh is broken" could mean any of them. See
  [ARCHITECTURE.md](docs/ARCHITECTURE.md) if you are not sure which one you were using.
- **What you ran.** The command, the cluster configuration file, and the relevant log
  output. [GETTING_STARTED.md](docs/GETTING_STARTED.md) is the reference for what a working
  invocation looks like.

If you are reporting a build failure, say whether you built from a clone or from a working
tree you had already used. Those are different situations, and the difference matters.

## Dual-use terms apply to contributions too

Read [DUAL_USE.md](DUAL_USE.md) before proposing anything. Two points bear on contributors
directly:

1. **Do not send anything you are not permitted to publish.** Export-controlled material,
   classified information, or code you do not hold the rights to must not enter this
   repository. Once something is in git history it is effectively permanent.
2. **Proposals that exist to enable the uses listed in
   [DUAL_USE.md §3](DUAL_USE.md#3-uses-this-project-rejects) will be declined**, including
   mass surveillance tooling and autonomous targeting.

## If you are invited to send code

Agreed in an issue first, so the shape is not a surprise to either of us.

- **Rust 2021**, `cargo fmt` defaults. Match the file you are editing; the codebase is not
  uniform, and consistency with its neighbours beats consistency with your preferences.
- **Prefer returning errors to panicking**, particularly anywhere that handles network
  input. A panic in a node process is a denial of service.
- **Use `tracing`, not `println!`.** Some older code still uses `println!`; new code should
  not add more.
- **Keep the Greek node vocabulary.** *komvos*, *kryphos*, *ypolo* and the rest are
  deliberate. See
  [ARCHITECTURE.md §7](docs/ARCHITECTURE.md#7-design-decisions-worth-knowing).
- **Wire types in `compute-abi` and `mesh_gateway_wire` are compatibility surfaces.**
  Changing one breaks every node type at once. Version the change rather than mutating a
  type in place; the `V1` suffixes exist for that reason.
- **Say what you changed and how you checked it.** If it touches cryptography, key handling,
  or the wire protocol, say so plainly. Review will be slower for those, and that is the
  point.

Run what you can before sending:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets
cargo test --workspace
```

Parts of the test suite need a running KMS cluster. If you could not run everything, say
which parts you did run rather than implying all of it passed.

## The vendored KMS

`kms/` is a modified copy of [Zama KMS v0.12.3](https://github.com/zama-ai/kms/tree/v0.12.3),
and it has its own rules. Read [kms/UPSTREAM.md](kms/UPSTREAM.md) first.

The short version: prefer changing ØMYSTIK code that calls into the KMS over changing the
KMS itself, because every local modification makes future upstream merges harder. Bugs that
also exist upstream belong to Zama, not here.

## Licensing your contribution

By submitting anything you agree it is licensed under **BSD-3-Clause-Clear** (see
[LICENSING.md](LICENSING.md)) and that you have the right to submit it. There is no separate
CLA to sign.

## Conduct

Be straightforward and courteous. Assume the other person is acting in good faith and is
short on time. Technical disagreement is welcome; personal hostility is not.
