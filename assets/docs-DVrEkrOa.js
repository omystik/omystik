const __vite__mapDeps=(i,m=__vite__mapDeps,d=(m.f||(m.f=["assets/mermaid.core-CRqxw8r8.js","assets/preload-helper-CMCUxjad.js","assets/rolldown-runtime-CbXtAM7H.js","assets/preload-helper-BXxBXVXa.css","assets/chunk-Y2CYZVJY-DsF7k-Jl.js","assets/src-BM-zdspv.js","assets/chunk-DU6HZSFF-CfJB67GD.js","assets/chunk-75Z2AOVW-HOtPtYXN.js","assets/dist-DMVjEMEa.js","assets/chunk-PWAF6VOD-DEYhnyxo.js","assets/chunk-GMAD6QVW-BxZcggta.js","assets/chunk-P2QGCYS3-Cdz_MCIP.js","assets/chunk-4HAMMTFA-DpxOOXwj.js","assets/rough.esm-Dy-Kn_BL.js","assets/chunk-GVQU2GXP-Bg8X1zLs.js","assets/chunk-OSK3NFVY-DyKwkH5W.js","assets/line-DQSrSau4.js","assets/path-fybaL0A-.js","assets/array-BifhSqXX.js","assets/graphlib-DS17s2tU.js","assets/chunk-L3NEJ4N5-Drm84v5B.js"])))=>i.map(i=>d[i]);
import{a as e,c as t,i as n,l as r,n as i,r as a,s as o,t as s}from"./preload-helper-CMCUxjad.js";var c=r(),l=t(),u=[{route:`/contributing/`,source:`CONTRIBUTING.md`,title:`Contributing to ØMYSTIK`,html:`<h1 id="contributing-to-ømystik">Contributing to ØMYSTIK</h1>
<p><strong>ØMYSTIK is not yet open to unsolicited pull requests.</strong></p>
<p>The project is pre-production, unaudited, and maintained by one person. Until the items in
<a href="/security/#known-limitations">SECURITY.md</a> are closed, code arriving faster than it can be
reviewed is a risk rather than a help. That is a statement about the project&#39;s capacity, not
about the quality of anyone&#39;s work.</p>
<p>This will change. When it does, this document changes with it.</p>
<hr>
<h2 id="what-is-welcome-now">What is welcome now</h2>
<p><strong>Bug reports.</strong> Something that does not behave as the documentation says, or does not build
from a clean clone. These are the most useful thing you can send.</p>
<p><strong>Questions about how it works.</strong> If the documentation left you guessing, that is a defect
in the documentation. Say where you got lost.</p>
<p><strong>Proposals.</strong> An idea, a design, a change you would like to make. Open an issue and
describe it before writing code, and we can agree the scope first. If it fits, you will be
invited to send the work as a pull request.</p>
<p><strong>Security defects go somewhere else.</strong> Report them privately, following
<a href="/security/">SECURITY.md</a>. Never open a public issue for a security defect, and never send
one as a pull request: a public fix for a live vulnerability discloses that vulnerability to
everyone who is watching.</p>
<h2 id="what-to-expect">What to expect</h2>
<p>One person maintains this alongside other work. Expect a reply in days rather than hours,
and sometimes longer. If an issue goes quiet for a fortnight, a polite nudge is reasonable
and not unwelcome.</p>
<p>Proposals that fit the project&#39;s direction may still be declined on timing. The roadmap is
in <a href="/readme/#what-works-and-what-doesnt">the README</a>, and the honest summary is that the
confidential-compute path works while the decentralised-AI layer above it has not been
started.</p>
<h2 id="before-you-open-an-issue">Before you open an issue</h2>
<p>Two things make an issue immediately actionable:</p>
<ul>
<li><strong>Which part.</strong> The crate, the node type, or the protocol. The three meshes behave very
differently, and &quot;the mesh is broken&quot; could mean any of them. See
<a href="/docs/architecture/">ARCHITECTURE.md</a> if you are not sure which one you were using.</li>
<li><strong>What you ran.</strong> The command, the cluster configuration file, and the relevant log
output. <a href="/docs/getting-started/">GETTING_STARTED.md</a> is the reference for what a working
invocation looks like.</li>
</ul>
<p>If you are reporting a build failure, say whether you built from a clone or from a working
tree you had already used. Those are different situations, and the difference matters.</p>
<h2 id="dual-use-terms-apply-to-contributions-too">Dual-use terms apply to contributions too</h2>
<p>Read <a href="/dual-use/">DUAL_USE.md</a> before proposing anything. Two points bear on contributors
directly:</p>
<ol>
<li><strong>Do not send anything you are not permitted to publish.</strong> Export-controlled material,
classified information, or code you do not hold the rights to must not enter this
repository. Once something is in git history it is effectively permanent.</li>
<li><strong>Proposals that exist to enable the uses listed in
<a href="/dual-use/#3-uses-this-project-rejects">DUAL_USE.md §3</a> will be declined</strong>, including
mass surveillance tooling and autonomous targeting.</li>
</ol>
<h2 id="if-you-are-invited-to-send-code">If you are invited to send code</h2>
<p>Agreed in an issue first, so the shape is not a surprise to either of us.</p>
<ul>
<li><strong>Rust 2021</strong>, <code>cargo fmt</code> defaults. Match the file you are editing; the codebase is not
uniform, and consistency with its neighbours beats consistency with your preferences.</li>
<li><strong>Prefer returning errors to panicking</strong>, particularly anywhere that handles network
input. A panic in a node process is a denial of service.</li>
<li><strong>Use <code>tracing</code>, not <code>println!</code>.</strong> Some older code still uses <code>println!</code>; new code should
not add more.</li>
<li><strong>Keep the Greek node vocabulary.</strong> <em>komvos</em>, <em>kryphos</em>, <em>ypolo</em> and the rest are
deliberate. See
<a href="/docs/architecture/#7-design-decisions-worth-knowing">ARCHITECTURE.md §7</a>.</li>
<li><strong>Wire types in <code>compute-abi</code> and <code>mesh_gateway_wire</code> are compatibility surfaces.</strong>
Changing one breaks every node type at once. Version the change rather than mutating a
type in place; the <code>V1</code> suffixes exist for that reason.</li>
<li><strong>Say what you changed and how you checked it.</strong> If it touches cryptography, key handling,
or the wire protocol, say so plainly. Review will be slower for those, and that is the
point.</li>
</ul>
<p>Run what you can before sending:</p>
<pre><code class="language-bash">cargo fmt --all
cargo clippy --workspace --all-targets
cargo test --workspace
</code></pre>
<p>Parts of the test suite need a running KMS cluster. If you could not run everything, say
which parts you did run rather than implying all of it passed.</p>
<h2 id="the-vendored-kms">The vendored KMS</h2>
<p><code>kms/</code> is a modified copy of <a href="https://github.com/zama-ai/kms/tree/v0.12.3">Zama KMS v0.12.3</a>,
and it has its own rules. Read <a href="https://github.com/omystik/omystik/blob/main/kms/UPSTREAM.md">kms/UPSTREAM.md</a> first.</p>
<p>The short version: prefer changing ØMYSTIK code that calls into the KMS over changing the
KMS itself, because every local modification makes future upstream merges harder. Bugs that
also exist upstream belong to Zama, not here.</p>
<h2 id="licensing-your-contribution">Licensing your contribution</h2>
<p>By submitting anything you agree it is licensed under <strong>BSD-3-Clause-Clear</strong> (see
<a href="/licensing/">LICENSING.md</a>) and that you have the right to submit it. There is no separate
CLA to sign.</p>
<h2 id="conduct">Conduct</h2>
<p>Be straightforward and courteous. Assume the other person is acting in good faith and is
short on time. Technical disagreement is welcome; personal hostility is not.</p>
`},{route:`/dual-use/`,source:`DUAL_USE.md`,title:`Dual-Use Notice and Responsible Use`,html:`<h1 id="dual-use-notice-and-responsible-use">Dual-Use Notice and Responsible Use</h1>
<blockquote>
<p><strong>§5 is provisional.</strong> A declaration covering this software is being prepared for
France&#39;s national cybersecurity agency (ANSSI), under décret n° 2007-663. That filing
adds to §5 rather than changing it: no export classification has been obtained, and an
ANSSI declaration is not one. The rest of this document is not affected. This note goes
when the declaration is filed.</p>
</blockquote>
<hr>
<h2 id="1-what-this-software-is">1. What this software is</h2>
<p>ØMYSTIK is a peer-to-peer mesh in which:</p>
<ul>
<li>nodes discover each other and have distinctive roles;</li>
<li>computation over data can be performed <strong>while the data stays encrypted</strong>, using
fully homomorphic encryption (FHE) and threshold key management;</li>
<li>content is encrypted (XChaCha20Poly1305) and addressed by content identifier;</li>
<li>content&#39;s metadata is hashed into Merkle structures so it can be queried without being published
in the clear;</li>
</ul>
<p>Those properties exist to protect the people whose data crosses the mesh. The same
properties make the system attractive to actors whose purposes are not legitimate. That
tension is inherent to strong cryptography and to resilient networking; it cannot be
engineered away, only acknowledged and managed.</p>
<h2 id="2-intended-uses">2. Intended uses</h2>
<p><em>These are the uses the architecture is aimed at. None of them is a claim that this release
is fit for them. <a href="#6-state-of-development">§6</a> states what has actually been built and
tested.</em></p>
<p>ØMYSTIK is intended for lawful use by organizations that need confidentiality and
resilience in constrained or contested environments. Representative examples:</p>
<p><strong>Civil</strong></p>
<ul>
<li>Disaster response and civil protection where fixed infrastructure has failed</li>
<li>Medical and humanitarian coordination involving patient or beneficiary data</li>
<li>Judicial and legal workflows with confidentiality obligations</li>
<li>Scientific collaboration on sensitive or pre-publication data</li>
<li>Industrial and economic-intelligence protection</li>
</ul>
<p><strong>Defense and security</strong></p>
<ul>
<li>Command, control and situational awareness for units operating with degraded links</li>
<li>Coalition data sharing where participants must collaborate without pooling raw data</li>
<li>Logistics and convoy coordination (see the OSATCON reference application)</li>
</ul>
<h2 id="3-uses-this-project-rejects">3. Uses this project rejects</h2>
<p>The maintainers do not support, and ask that you do not use this software for:</p>
<ul>
<li><strong>Mass or indiscriminate surveillance</strong>, including bulk collection or analysis of
communications or movements of people who are not the subject of a lawful,
particularized authorization.</li>
<li><strong>Targeting of civilians</strong>, or any use forming part of an operation that violates
international humanitarian law.</li>
<li><strong>Suppression of human rights.</strong> Surveilling, locating, or building profiles of
journalists, lawyers, human-rights defenders, political dissidents, minority groups,
or their families.</li>
<li><strong>Autonomous use of force.</strong> ØMYSTIK carries data and computes over it. It must not be
wired into a weapon-release or engagement decision that no accountable human authorizes.</li>
<li><strong>Concealing unlawful activity</strong>, including using the mesh&#39;s confidentiality properties
to evade lawful oversight of your own conduct.</li>
<li><strong>Circumventing sanctions or export controls</strong> (see <a href="#5-export-control-and-sanctions">§5</a>).</li>
</ul>
<p>This section is a statement of the project&#39;s position and a condition of the
maintainers&#39; support. It is <strong>not</strong> an additional restriction on the software license:
the BSD-3-Clause-Clear license in <a href="/licensing/">LICENSING.md</a> governs your legal rights
and grants freedoms this section does not revoke. What the maintainers control is
whether they will assist, accept contributions from, or collaborate with a given
deployment, and that is withheld from uses listed above.</p>
<h2 id="4-obligations-of-the-deployer">4. Obligations of the deployer</h2>
<p>If you deploy ØMYSTIK you take on responsibilities the software cannot discharge for you:</p>
<ol>
<li><strong>Legal basis.</strong> Establish that your processing has a lawful basis in every
jurisdiction where a node runs, and that operating an encrypted mesh is itself lawful
there. Some jurisdictions restrict cryptography, mesh networking, or specific radio
bearers.</li>
<li><strong>Authorization and accountability.</strong> Maintain records of who authorized each
deployment and each class of data processed. The mesh is deliberately
privacy-preserving; it will not produce an audit trail of content on your behalf.</li>
<li><strong>Key custody.</strong> Threshold FHE distributes trust across parties. If one operator
controls enough parties to reconstruct, the threshold guarantee is nominal. Document
your party distribution and who runs each one.</li>
<li><strong>Downstream notice.</strong> If you redistribute or integrate ØMYSTIK, carry this document
forward.</li>
</ol>
<h2 id="5-export-control-and-sanctions">5. Export control and sanctions</h2>
<p>ØMYSTIK incorporates strong cryptography, including FHE, and is documented for defense
applications. Cryptographic software and defense-related technology are export-controlled
in many jurisdictions.</p>
<p><strong>The maintainers have not obtained an export classification for this software and make
no representation about its export status.</strong> Nothing here is legal advice.</p>
<p>Regimes that may apply, depending on where you are and where the software goes:</p>
<ul>
<li><strong>EU.</strong> Regulation (EU) 2021/821 (dual-use recast). Cryptography falls under
Category 5 Part 2; some defense-adjacent items fall under national military lists.</li>
<li><strong>France.</strong> Two separate obligations. The <em>Agence nationale de la sécurité des systèmes
d&#39;information</em> (ANSSI) regime covers supplying, transferring and importing a means of
cryptology, and for most functions is satisfied by declaration. Taking the software
outside the EU is a different question, handled by the <em>Service des biens à double usage</em>
(SBDU) under the dual-use regulation. A declaration made to ANSSI is not an export
classification and authorizes no export.</li>
<li><strong>United States.</strong> The Export Administration Regulations (EAR), Category 5 Part 2 for
encryption; ITAR if a deployment makes the software a defense article.</li>
</ul>
<p>These regimes share common technical categories because they all implement the Wassenaar Arrangement, the multilateral basis behind most dual-use export control.</p>
<p>Before you export, re-export, or make this software available across a border, or provide
it to a sanctioned party or embargoed destination, obtain your own classification and
authorization. If you contribute code, do not contribute anything you are not permitted to
publish.</p>
<h2 id="6-state-of-development">6. State of development</h2>
<p>Three things are worth stating plainly for anyone assessing the system, because the
distance between what ØMYSTIK is designed for and what it has been shown to do is large.</p>
<p><strong>Only ordinary IP networks have been exercised.</strong> Every test of this release ran over
Wi-Fi and conventional internet links, from machines on a local network out to compute
instances on Google Cloud Platform. Nothing has been developed for the bearers a
deployment in the field would actually use: LoRa, satellite links, tactical radio, or any
path that is low-bandwidth, high-latency, or intermittent by nature. The mesh has never run
over one, so how it behaves there is unknown rather than degraded. Read the resilience
properties described anywhere in this repository as design intent that remains to be
demonstrated.</p>
<p><strong>Decentralized AI is not implemented.</strong> Federated learning, deep FHE inference, and
shared decentralized compute for training appear in the project&#39;s roadmap and in
earlier project descriptions. They are <strong>not in this codebase</strong>. Do not represent
ØMYSTIK as providing them today.</p>
<p><strong>This is pre-production software.</strong> See <a href="/security/">SECURITY.md</a> for the current
security posture, which includes an <code>insecure</code> feature enabled in the default build and
test configurations that derive keys from short deterministic seeds. It has not undergone
independent security audit or cryptographic review. Do not protect real lives or real
secrets with it in its present state.</p>
<p>Taken together: the defense uses in <a href="#2-intended-uses">§2</a> describe where the architecture
points, not a capability on offer. Substantial development separates this release from any
of them, and the project is best understood at this stage as experimental research code
published so that the approach can be examined.</p>
<h2 id="7-reporting-misuse">7. Reporting misuse</h2>
<p>If you become aware of a deployment of ØMYSTIK used for any purpose in <a href="#3-uses-this-project-rejects">§3</a>, contact the
maintainer (see <a href="/security/">SECURITY.md</a>). Reports concerning an active threat to
someone&#39;s safety should go to the appropriate authority first; the maintainers cannot
intervene operationally.</p>
<hr>
<p><em>This document covers responsible use. For vulnerability reporting see
<a href="/security/">SECURITY.md</a>; for license terms and the Zama commercial-use obligation see
<a href="/licensing/">LICENSING.md</a>.</em></p>
`},{route:`/licensing/`,source:`LICENSING.md`,title:`Licensing`,html:`<h1 id="licensing">Licensing</h1>
<p>ØMYSTIK is licensed <strong>BSD-3-Clause-Clear</strong>, matching the license of the ZAMA components it
builds on.</p>
<hr>
<h2 id="1-ømystiks-own-license">1. ØMYSTIK&#39;s own license</h2>
<p>BSD-3-Clause-Clear (the &quot;Clear BSD License&quot;) is a permissive license with one notable
addition over plain BSD-3-Clause: it states explicitly that <strong>no patent rights are
granted</strong>. You get broad freedom to use, modify, and redistribute the code, including
commercially, provided you retain the copyright notice, the license text, and the
disclaimer, and do not use the authors&#39; names to endorse derived products.</p>
<p>Copyright holder: Michael John Hatchi.</p>
<p>Full text: <a href="https://spdx.org/licenses/BSD-3-Clause-Clear.html">https://spdx.org/licenses/BSD-3-Clause-Clear.html</a></p>
<h2 id="2-zama-components-commercial-use-requires-an-agreement">2. ZAMA components: commercial use requires an agreement</h2>
<p>ØMYSTIK vendors and links ZAMA&#39;s key-management and FHE stack. This is the single most
important licensing obligation in the project, and it is easy to overlook because the
license itself is permissive.</p>
<p><strong>ZAMA&#39;s BSD-3-Clause-Clear licensing of TFHE-rs, Concrete, Concrete-ML, threshold-FHE and
the KMS applies to non-commercial and evaluation use. ZAMA requires a separate commercial patent
license agreement for commercial use. Commercial terms, including any applicable usage measurement and remuneration mechanism, are agreed directly with ZAMA.</strong></p>
<p>If you intend to build a product or service on ØMYSTIK, contact ZAMA and conclude that
agreement. It is your obligation, not the ØMYSTIK maintainers&#39;. A permissive license on
this repository does not and cannot waive ZAMA&#39;s terms. See <a href="https://www.zama.ai/">https://www.zama.ai/</a> and
the license text shipped with each ZAMA component.</p>
<h3 id="vendored-kms">Vendored KMS</h3>
<p>The <code>kms/</code> directory contains a vendored copy of ZAMA&#39;s KMS, taken from:</p>
<blockquote>
<p><a href="https://github.com/zama-ai/kms/tree/v0.12.3">https://github.com/zama-ai/kms/tree/v0.12.3</a></p>
</blockquote>
<p>It is not a submodule; it is a copy carrying local modifications for libp2p transport
(see <a href="/docs/architecture/">ARCHITECTURE.md</a>).</p>
<p>Consequences:</p>
<ul>
<li>ZAMA&#39;s copyright notice and license text <strong>must</strong> ship with it.</li>
<li>Upstream security fixes do not arrive automatically. Track
<a href="https://github.com/zama-ai/kms">https://github.com/zama-ai/kms</a> releases and port fixes deliberately.</li>
</ul>
<p>ZAMA components in use, via the workspace manifest:</p>
<table>
<thead>
<tr>
<th>Component</th>
<th>Version</th>
<th>Source</th>
</tr>
</thead>
<tbody><tr>
<td><code>kms</code> (core service)</td>
<td>vendored</td>
<td><code>kms/core/service</code>, upstream v0.12.3</td>
</tr>
<tr>
<td><code>kms-grpc</code></td>
<td>vendored</td>
<td><code>kms/core/grpc</code></td>
</tr>
<tr>
<td><code>threshold-fhe</code></td>
<td>vendored</td>
<td><code>kms/core/threshold</code></td>
</tr>
<tr>
<td><code>observability</code></td>
<td>vendored</td>
<td><code>kms/observability</code></td>
</tr>
<tr>
<td><code>bc2wrap</code></td>
<td>vendored</td>
<td><code>kms/bc2wrap</code></td>
</tr>
<tr>
<td><code>tfhe</code></td>
<td>1.4.0-alpha.3</td>
<td>crates.io</td>
</tr>
<tr>
<td><code>tfhe-csprng</code></td>
<td>0.7.0</td>
<td>crates.io</td>
</tr>
<tr>
<td><code>tfhe-versionable</code></td>
<td>0.6.2</td>
<td>crates.io</td>
</tr>
<tr>
<td><code>tfhe-zk-pok</code></td>
<td>0.7.3</td>
<td>crates.io</td>
</tr>
</tbody></table>
<h2 id="3-other-third-party-components">3. Other third-party components</h2>
<table>
<thead>
<tr>
<th>Component</th>
<th>License</th>
<th>Role</th>
</tr>
</thead>
<tbody><tr>
<td><a href="https://github.com/libp2p/rust-libp2p">rust-libp2p</a> 0.54.1</td>
<td>MIT</td>
<td>P2P transport, DHT, rendezvous, streams, request-response</td>
</tr>
<tr>
<td><code>chacha20poly1305</code> (RustCrypto)</td>
<td>Apache-2.0 OR MIT</td>
<td>XChaCha20Poly1305 content encryption</td>
</tr>
<tr>
<td><code>blake3</code></td>
<td>Apache-2.0 OR CC0-1.0</td>
<td>Content and transfer hashing</td>
</tr>
<tr>
<td><code>rs_merkle</code></td>
<td>MIT</td>
<td>Classic Merkle tree for metadata (MID)</td>
</tr>
<tr>
<td><code>monotree</code></td>
<td>MIT</td>
<td>Sparse Merkle tree for MID→CID mapping</td>
</tr>
<tr>
<td><code>sled</code> (via monotree, <code>db_sled</code>)</td>
<td>Apache-2.0 / MIT</td>
<td>Embedded persistence backend for the sparse Merkle tree</td>
</tr>
<tr>
<td><code>tokio</code>, <code>tower</code>, <code>tonic</code>, <code>tracing</code></td>
<td>MIT</td>
<td>Async runtime, gRPC, instrumentation</td>
</tr>
<tr>
<td><code>ndarray</code></td>
<td>Apache-2.0 OR MIT</td>
<td>Numeric arrays</td>
</tr>
<tr>
<td>W3C DID specifications</td>
<td>W3C Document License</td>
<td>Identity data model (planned; not implemented)</td>
</tr>
</tbody></table>
<h3 id="supply-chain-annotations-in-the-manifest">Supply-chain annotations in the manifest</h3>
<p><code>Cargo.toml</code> annotates most dependencies with a maintainer-risk assessment
(<code>LOW RISK: RustCrypto org</code>, <code>HIGH RISK: Individual maintainer</code>, and so on). These are the
maintainer&#39;s own judgements, inherited in part from ZAMA&#39;s manifest.</p>
<h2 id="4-contributor-licensing">4. Contributor licensing</h2>
<p>By submitting a contribution you agree it is licensed under BSD-3-Clause-Clear and that
you have the right to submit it. There is no separate CLA. See
<a href="/contributing/">CONTRIBUTING.md</a>.</p>
<p>Do not contribute code you are not permitted to publish, export-controlled material in
particular. See <a href="/dual-use/#5-export-control-and-sanctions">DUAL_USE.md §5</a>.</p>
<h2 id="5-where-the-license-texts-live">5. Where the license texts live</h2>
<table>
<thead>
<tr>
<th>File</th>
<th>Covers</th>
</tr>
</thead>
<tbody><tr>
<td><a href="https://github.com/omystik/omystik/blob/main/LICENSE">LICENSE</a></td>
<td>ØMYSTIK&#39;s own code, BSD-3-Clause-Clear</td>
</tr>
<tr>
<td><a href="https://github.com/omystik/omystik/blob/main/kms/LICENSE">kms/LICENSE</a></td>
<td>ZAMA&#39;s KMS, vendored under <code>kms/</code>. BSD 3-Clause Clear, © 2024 ZAMA</td>
</tr>
<tr>
<td><a href="https://github.com/omystik/omystik/blob/main/kms/UPSTREAM.md">kms/UPSTREAM.md</a></td>
<td>What in <code>kms/</code> diverges from upstream v0.12.3, and who to report defects to</td>
</tr>
</tbody></table>
<p>If you redistribute ØMYSTIK, both license files travel with it. Every other dependency is
permissively licensed, see <a href="#3-other-third-party-components">§3</a>.<br>
So the only obligation that needs a decision rather than a notice is ZAMA&#39;s commercial-use agreement in
<a href="#2-zama-components-commercial-use-requires-an-agreement">§2</a>.</p>
<hr>
<p><em>Responsible-use terms are in <a href="/dual-use/">DUAL_USE.md</a>; security posture in
<a href="/security/">SECURITY.md</a>.</em></p>
`},{route:`/readme/`,source:`README.md`,title:`README`,html:`<div align="center"><img src="/docs-media/image/black_omystik_logo_zoomed.png" alt="ØMYSTIK" width="1020"><br>
<br>
<br><p><strong>An open source peer-to-peer mesh that paves the way for productivity in a private setting while in extreme environment:<br>
carries and computes data it cannot read, for peers it does not trust.</strong></p>
<p><img src="https://img.shields.io/badge/version-v0.2.1-green.svg" alt="Version: v0.2.1">
<a href="/licensing/"><img src="https://img.shields.io/badge/License-BSD--3--Clause--Clear-blue.svg" alt="License: BSD-3-Clause-Clear"></a>
<a href="/security/"><img src="https://img.shields.io/badge/status-pre--production-orange.svg" alt="Status: pre-production"></a>
<a href="/dual-use/"><img src="https://img.shields.io/badge/use-dual--use-important.svg" alt="Dual-use"></a></p>
</div><hr>
<blockquote>
<p><br> <strong>⚠️ IMPORTANT</strong>
<br><br>
<strong>Pre-production and unaudited.</strong>
<br> ØMYSTIK has not undergone independent security or
cryptographic review, and the default build enables an <code>insecure</code> feature flag. Do not
use it to protect data whose disclosure would harm someone. See
<a href="/security/">SECURITY.md</a>.</p>
<p><strong>Dual-use technology.</strong>
<br> This software is documented for both civil and defense
applications, incorporates strong cryptography -- <a href="https://csrc.nist.gov/csrc/media/presentations/2026/mpts2026-2b3/images-media/mpts2026-2b3-slides-zama-fhe-smart.pdf">ZAMA&#39;s TFHE and MPC</a> -- which has been submitted to <a href="https://csrc.nist.gov/projects/threshold-cryptography">NIST&#39;s theshold cryptography call</a>. Read <a href="/dual-use/">DUAL_USE.md</a> before deploying.</p>
<p><strong>License BSD-3-Clause-Clear: research and development vs commercial use</strong>
<br> ØMYSTIK builds on ZAMA&#39;s TFHE and KMS stack. While research and development are free of use, commercial use requires a ZAMA agreement. See <a href="/licensing/">LICENSING.md</a>.</p>
</blockquote>
<hr>
<br><h2 id="how-it-works">How it works</h2>
<p>Networks in the field are interfered with, jammed, attacked, and cut. One usual answer
encrypts data in transit and at rest, then decrypts it to do anything useful, exposing it
at exactly the moment it matters.</p>
<p>ØMYSTIK takes a different approach. Each terminal, laptop, phone, server, vehicle, UxV,
sensor becomes a node in an encrypted mesh that has no knowledge of the transiting data.</p>
<ul>
<li><p><strong>Layer 0 - MYSTIK&gt;P2P</strong> (<a href="#Lineage">see lineage.</a>)</p>
<ul>
<li><em>autonomous discovery</em>: nodes join the mesh through rendezvous points; stored content
and key replicas are located through the Kademlia DHT.</li>
<li><em>persistently stores without reading</em>: files are encrypted with XChaCha20Poly1305
before they touch disk, and their metadata are hashed into a Merkle root (MID).
Users locate a file by knowing its metadata; the mesh keeps no copy of the
decryption key.</li>
</ul>
</li>
<li><p><strong>Layer 1 - computes without decrypting</strong></p>
<ul>
<li>executors run <em>smart programs</em> over ciphertext with an evaluation key only: they
cannot read their own inputs or outputs.</li>
<li>programs have a stable identity, a manifest and a typed ABI, and can consume
live encrypted inputs over a session.</li>
</ul>
</li>
<li><p><strong>Layer 2 - decrypts only by quorum</strong></p>
<ul>
<li>FHE keys are generated and held in threshold across independent MPC parties
(Zama&#39;s KMS, moved from gRPC to libp2p): no single party can decrypt.</li>
<li>the key mesh is reachable only through a gateway that aggregates partial results
but holds no share and never sees an MPC round.</li>
</ul>
</li>
</ul>
<h2 id="roadmap">Roadmap</h2>
<ul>
<li><p><strong>Layer 3 - physarum solver</strong> in <code>/mesh-c</code></p>
<ul>
<li>computing needs are channelled toward computing nodes, instead of executors being
dialled from configuration.</li>
</ul>
</li>
<li><p><strong>Layer 4 - <code>/mesh-p</code></strong></p>
<ul>
<li>a mesh of gateway nodes, so that Pylon is no longer a single bottleneck.</li>
</ul>
</li>
<li><p><strong>Layer 5 - self-repairing mesh</strong></p>
<ul>
<li>the mesh rebalances itself according to the roles its nodes need to fill.</li>
</ul>
</li>
<li><p><strong>Layer 6 - Decentralized IDentification</strong></p>
</li>
</ul>
<p>Built on major open sources development: Protocol Labs&#39; <a href="https://github.com/libp2p/rust-libp2p">rust-libp2p</a>,
<a href="https://github.com/ZAMA-ai/kms">ZAMA&#39;s KMS</a> and TFHE-rs.</p>
<h2 id="type-of-nodes-and-meshes">Type of Nodes and Meshes</h2>
<p><strong>GRΣΣK names say it all.</strong></p>
<pre><code>             _
  __  __   _| |_ ___  _ __   ___  _ __ ___   ___  ___                  
 / _\` | | | | __/ _ \\| &#39;_ \\ / _ \\| &#39;_ \` _ \\ / _ \\/ __|                   
| (_| | |_| | || (_) | | | | (_) | | | | | | (_) \\__ \\  - autonomous          
 \\__,_|\\__,_|\\__\\___/|_| |_|\\___/|_| |_| |_|\\___/|___/                    
</code></pre>
<pre><code> _              _           _  _ _
| | _____ _ __ | |_ _ __  / /___\\ \\                                      
| |/ / _ \\ &#39;_ \\| __| &#39;__ | |/ _ \\| |                                    
|   &lt;  __/ | | | |_| |   | | (_) | |  - centre                                       
|_|\\_\\___|_| |_|\\__|_| _ | |\\___/| |                                       
                          \\_\\  _/_/
</code></pre>
<pre><code> _
| | _____  _ __ _____   _____  ___                                          
| |/ / _ \\| &#39;_ \` _ \\ \\ / / _ \\/ __|                                        
|   &lt; (_) | | | | | \\ V / (_) \\__ \\  - node                                      
|_|\\_\\___/|_| |_| |_|\\_/ \\___/|___/
</code></pre>
<pre><code> _                     _
| | ___ __ _   _ _ __ | |_    __   ___                                   
| |/ / &#39;__| | | | &#39;_ \\| &#39;_ \\ / _ \\/ __|                                    
|   &lt;| |  | |_| | |_) | | | | (_) \\__ \\  - hidden                               
|_|\\_\\_|   \\__, | .__/|_| |_|\\___/|___/                                    
           |___/|_|                                                            
</code></pre>
<pre><code> _                     _                         __ _       
| | ___ __ _   _ _ __ | |_ ___   __ _ _ __ __ _ / _(_) __ _ 
| |/ / &#39;__| | | | &#39;_ \\| __/ _ \\ / _\` | &#39;__/ _\` | |_| |/ _\` |
|   &lt;| |  | |_| | |_) | || (_) | (_| | | | (_| |  _| | (_| |  - cryptography
|_|\\_\\_|   \\__, | .__/ \\__\\___/ \\__, |_|  \\__,_|_| |_|\\__,_|
           |___/|_|             |___/                                                                 
</code></pre>
<pre><code>       _                                                 _       _   _
 _ __ | | ___  __ _ _ __ ___   __ _           _ __   ___| | __ _| |_(_)___  
| &#39;_ \\| |/ _ \\/ _\` | &#39;_ \` _ \\ / _\` |  _____  | &#39;_ \\ / _ \\ |/ _\` | __| / __|
| |_) | |  __/ (_| | | | | | | (_| | |_____| | |_) |  __/ | (_| | |_| \\__ \\  - mesh - client
| .__/|_|\\___|\\__, |_| |_| |_|\\__,_|         | .__/ \\___|_|\\__,_|\\__|_|___/
|_|           |___/                          |_|                         
</code></pre>
<pre><code>             _
 _ __  _   _| | ___  _ __                                                
| &#39;_ \\| | | | |/ _ \\| &#39;_ \\                                              
| |_) | |_| | | (_) | | | |  - gate                                           
| .__/ \\__, |_|\\___/|_| |_|                 
|_|    |___/            
</code></pre>
<pre><code>                     _                                                         
 ___ _   _ _ __   __| | ___  ___ _ __ ___   ___  ___                
/ __| | | | &#39;_ \\ / _\` |/ _ \\/ __| &#39;_ \` _ \\ / _ \\/ __|                   
\\__ \\ |_| | | | | (_| |  __/\\__ \\ | | | | | (_) \\__ \\  - link                      
|___/\\__, |_|_|_|\\__,_|\\___||___/_| |_| |_|\\___/|___/
</code></pre>
<pre><code>                   _        __     _                       __                  
 _   _  ___   ___ | | ___  / /__ _(_)___ _ __ ___   ___  __\\ \\ 
| | | | &#39;_ \\ / _ \\| |/ _ \\| |/ _\` | / __| &#39;_ \` _ \\ / _ \\/ __| |            
| |_| | |_) | (_) | | (_) | | (_| | \\__ \\ | | | | | (_) \\__ \\ |  - computation
 \\__, | .__/ \\___/|_|\\___/| |\\__, |_|___/_| |_| |_|\\___/|___/ |           
 |___/|_|                  \\_\\___/                         /_/                 
</code></pre>
<hr>
<br><blockquote>
<p><strong>Mesh A: transport, sensor mesh, encrypted storage</strong> (<code>/mesh-a</code>)</p>
</blockquote>
<ul>
<li>Syndesmos: rendezvous and bootstrap node at a fixed address, other nodes register here to be discovered.</li>
<li>Autonomos: sensor node.<ul>
<li>registers with Syndesmos under <code>/mesh-a</code>;</li>
<li>captures and encrypts data thanks to the shared public key;</li>
<li>pushes encrypted inputs to smart-program sessions on Ypolo for future FHE computations;</li>
<li>can store encrypted content and serve it to peers, located through MIDs published to the DHT;</li>
</ul>
</li>
<li>Kentr: the control plane, with <code>Admin</code> / <code>NonAdmin</code> roles.<ul>
<li>registers with Syndesmos under <code>/mesh-a</code>;</li>
<li>orchestrates key artifacts: rehydrates the keyset from Pylon, then serves replicas
to Mesh A peers;</li>
<li>requests threshold decryption through Pylon, never from Kryphos directly;</li>
<li>deploys smart programs to Ypolo, opens sessions, pushes their configuration and polls
their output.</li>
</ul>
</li>
</ul>
<br><blockquote>
<p><strong>Mesh B: Multi-party computation making the Key Management System running</strong></p>
</blockquote>
<ul>
<li>Kryphos: one MPC party running Zama&#39;s KMS over libp2p instead of gRPC.<ul>
<li><ul>
<li>generates the CRS and the keys (public key, server key) as multi-party protocols; the private key stays in threshold;</li>
</ul>
</li>
<li>performs threshold decryption;</li>
<li>face A, internal: MPC rounds with the other parties (<code>/threshold-fhe/1</code>) via peer-to-peer libp2p;</li>
<li>face B, external: talks to Pylon and nothing else (<code>/mesh/gateway/crypto/1</code>).</li>
</ul>
</li>
</ul>
<br><blockquote>
<p><strong>Mesh C:  confidential compute</strong> (<code>/mesh-c</code>)</p>
</blockquote>
<ul>
<li>Ypolo: smart-program executor.<ul>
<li>Registers to Syndesmos under <code>/mesh-c</code>;</li>
<li>receives encrypted inputs from Autonomos nodes and session commands from Kentr;</li>
<li>runs smart programs as fully homomorphic computations over the ciphertext, with the
server key (evaluation key) only: it cannot read its own inputs or outputs;</li>
<li>two executors: in-process TFHE evaluation, or an external process implementing the
program (eg.: how the OSATCON alert program is deployed);</li>
<li>returns encrypted outputs with a signed receipt.</li>
</ul>
</li>
</ul>
<br><blockquote>
<p><strong>The gateway, future Mesh P</strong></p>
</blockquote>
<ul>
<li>Pylon:<ul>
<li>discovered by Mesh A nodes through libp2p identify, not configured;</li>
<li>Face A: jobs and blobs with Mesh A nodes;</li>
<li>Face B: fans crypto operations out to each Kryphos party&#39;s face B and aggregates the
partial results; the only node holding the Kryphos addresses;</li>
<li>currently holds the key artifacts generated by Mesh B and serves them over Face A.</li>
</ul>
</li>
</ul>
<h2 id="architecture-at-a-glance">Architecture at a glance</h2>
<p>Currrently, three meshes with distinct trust properties, bridged by a gateway:</p>
<pre><code>   MESH A  /mesh-a                      MESH C  /mesh-c
   transport &amp;                          confidential compute
   encrypted gathered data
   ┌──────────────────────────┐         ┌──────────────────┐
   │ syndesmos   rendezvous   │         │ ypolo            │
   │ autonomos   sensor peer  │◄───────►│   smart-program  │
   │ kentr       control      │  Face A │   executor       │
   └────────────┬─────────────┘         └──────────────────┘
                │ Face A: jobs + blobs
                ▼
        ┌───────────────┐
        │    pylon      │   the only bridge between
        │   GATEWAY     │   the mesh and the key material
        └───────┬───────┘
                │ Face B: cryptographic ops
                ▼
   ┌──────────────────────────────┐
   │ MESH B  /threshold-fhe/1     │
   │ kryphos                      │
   | MPC party 1 (p1) … p4        │   threshold FHE / KMS
   │                              │   (ZAMA KMS over libp2p)
   │                              │
   └──────────────────────────────┘
</code></pre>
<p>Ypolo computes on ciphertext and holds no decryption key.
No single Kryphos party can reconstruct.
Pylon aggregates partial results but holds no share.
Full detail in <a href="/docs/architecture/">docs/ARCHITECTURE.md</a>.</p>
<h2 id="repository-layout">Repository layout</h2>
<table>
<thead>
<tr>
<th>Path</th>
<th>Contents</th>
</tr>
</thead>
<tbody><tr>
<td><code>libp2p_common/</code></td>
<td>Swarm behaviours for all three meshes; node types and protocol constants</td>
</tr>
<tr>
<td><code>content-hashing/</code>, <code>metadata-mrkl/</code>, <code>persistency_utils/</code>, <code>db-access/</code></td>
<td>Encrypted persistent storage: ciphers, Merkle metadata, orchestration</td>
</tr>
<tr>
<td><code>kryptografia/</code></td>
<td>TFHE encryption helpers and KMS ciphertext types</td>
</tr>
<tr>
<td><code>compute-abi/</code>, <code>compute-core/</code>, <code>compute-program/</code>, <code>compute-client/</code></td>
<td>Smart-program framework: wire types, executor trait, program trait, client SDK</td>
</tr>
<tr>
<td><code>mesh_gateway_wire/</code>, <code>meshes_config/</code></td>
<td>Face A / Face B wire protocol; TOML cluster configuration</td>
</tr>
<tr>
<td><code>node_syndesmos/</code>, <code>node_autonomos/</code>, <code>node_kentr/</code></td>
<td>Mesh A nodes</td>
</tr>
<tr>
<td><code>node_kryphos/</code></td>
<td>Mesh B node, ZAMA KMS party over libp2p</td>
</tr>
<tr>
<td><code>node_ypolo/</code></td>
<td>Mesh C node, smart-program executor</td>
</tr>
<tr>
<td><code>node_pylon/</code></td>
<td>The gateway</td>
</tr>
<tr>
<td><code>plegma-fhe-pelatis/</code></td>
<td>FHE client: encrypt, decrypt, key material, artifact cache</td>
</tr>
<tr>
<td><code>osatcon/</code></td>
<td>Reference application: encrypted convoy/satellite exposure alerting</td>
</tr>
<tr>
<td><code>kms/</code></td>
<td><strong>Vendored</strong> ZAMA KMS (<a href="https://github.com/ZAMA-ai/kms/tree/v0.12.3">v0.12.3</a>). Modified for libp2p transport, see <a href="https://github.com/omystik/omystik/blob/main/kms/UPSTREAM.md">UPSTREAM.md</a></td>
</tr>
<tr>
<td><code>nodeA/</code> … <code>nodeN/</code></td>
<td>Per-node runtime data trees, not crates. See <a href="/docs/node-layout/">NODE_LAYOUT.md</a></td>
</tr>
</tbody></table>
<p>Per-crate reference: <a href="/docs/crates/">docs/CRATES.md</a>.</p>
<h2 id="getting-started">Getting started</h2>
<p>Requires a recent stable Rust toolchain. The workspace is large and the FHE dependencies
are heavy, so expect a long first build.</p>
<pre><code class="language-bash">cargo build --workspace
</code></pre>
<p>Bringing up a local cluster means starting a rendezvous node, an MPC cluster, a gateway,
and an executor in that order. Step-by-step in
<a href="/docs/getting-started/">docs/GETTING_STARTED.md</a>.</p>
<h2 id="what-works-and-what-doesnt">What works, and what doesn&#39;t</h2>
<p>Implemented and running:</p>
<ul>
<li>Encrypted persistent storage with queryable hashed metadata (Mesh A)</li>
<li>Threshold FHE key management over libp2p (Mesh B)</li>
<li>Confidential computation over ciphertext with signed receipts (Mesh C)</li>
<li>The Pylon gateway and its two faces</li>
<li>OSATCON, end to end, on constrained hardware including Raspberry Pi</li>
</ul>
<p><strong>Tested over Wi-Fi and conventional internet only</strong>, from separate devices on a shared Wi-Fi network out to compute
instances on Google Cloud Platform. LoRa, satellite and other field bearers are not
supported and have never been tried, so the resilience this architecture is designed for
remains to be demonstrated. See
<a href="/dual-use/#6-state-of-development">DUAL_USE.md §6</a>.</p>
<p>Not implemented, despite appearing in earlier project descriptions:</p>
<ul>
<li><strong>DID-based Zero-Trust identity</strong>: design intent, no code, roadmap</li>
<li><strong>Federated learning</strong> and <strong>deep FHE inference</strong>: roadmap</li>
<li><strong>Decentralised compute sharing for model training</strong>: roadmap</li>
<li>mDNS discovery, DoS mitigation, biomimetic self-healing behaviour</li>
</ul>
<p>The confidential-compute path of store, encrypt, compute under FHE and decrypt through a
threshold works end to end today. The decentralised-AI layer on top of it has not been
started.</p>
<h2 id="documentation">Documentation</h2>
<table>
<thead>
<tr>
<th>Document</th>
<th></th>
</tr>
</thead>
<tbody><tr>
<td><a href="/docs/architecture/">ARCHITECTURE.md</a></td>
<td>Meshes, protocols, data flows, design decisions</td>
</tr>
<tr>
<td><a href="/docs/crates/">CRATES.md</a></td>
<td>Per-crate reference</td>
</tr>
<tr>
<td><a href="/docs/getting-started/">GETTING_STARTED.md</a></td>
<td>Build and run a local cluster</td>
</tr>
<tr>
<td><a href="/docs/node-layout/">NODE_LAYOUT.md</a></td>
<td>Runtime data tree a storage node needs</td>
</tr>
<tr>
<td><a href="/docs/cryptography/">CRYPTOGRAPHY.md</a></td>
<td>Every primitive used, what it protects, where keys live</td>
</tr>
<tr>
<td><a href="/security/">SECURITY.md</a></td>
<td>Vulnerability reporting, known limitations, hardening</td>
</tr>
<tr>
<td><a href="/dual-use/">DUAL_USE.md</a></td>
<td>Responsible use, export control</td>
</tr>
<tr>
<td><a href="/licensing/">LICENSING.md</a></td>
<td>Licence terms and third-party obligations</td>
</tr>
<tr>
<td><a href="/contributing/">CONTRIBUTING.md</a></td>
<td>What is welcome now: issues and proposals, not yet pull requests</td>
</tr>
</tbody></table>
<h2 id="lineage">Lineage</h2>
<p>ØMYSTIK follows three earlier projects by the same author:</p>
<ul>
<li><strong>MYSTIK&gt;p2p</strong> (Nov 2024 to Mar 2025), persistent encrypted storage in a peer-to-peer
network with queryable hashed metadata. Its storage model is Mesh A today; its original
README is kept at <a href="/readme-mystik-p2p/">README_mystik-p2p.md</a>.</li>
<li><strong>FHE.IRM</strong> (spring 2024), a decentralized application managing access rights and
updates to confidential documents on public blockchain Ethereum (ZAMA&#39;s fhEVM).</li>
<li><strong>FHE.Chess</strong> (2023), a Chess application where the AI opponent (deep learning models),
two compiled (ZAMA&#39;s Concrete ML) CNN models, infers encrypted move based on encrypted chessboard
thanks to FHE. (<a href="https://github.com/vrona/FHE.Chess">see FHE.Chess</a>)</li>
</ul>
<h2 id="licence">Licence</h2>
<p>BSD-3-Clause-Clear. Copyright © 2024-2026 Michael John HATCHI.</p>
<p>Commercial use of the ZAMA components this project depends on requires a separate
agreement with <a href="https://www.ZAMA.ai/">ZAMA</a>. See <a href="/licensing/">LICENSING.md</a>. This is a
real obligation, not a formality.</p>
`},{route:`/readme-mystik-p2p/`,source:`README_mystik-p2p.md`,title:`MYSTIK>P2P`,html:`<h1 id="mystikp2p">MYSTIK&gt;P2P</h1>
<blockquote>
<p><strong>This is a historical document.</strong> It is the README of MYSTIK&gt;p2p (Nov 2024 to Mar 2025),
the project ØMYSTIK grew out of, kept because it is the original description of the
MID/CID storage model that Mesh A still uses. It describes that earlier system, not
ØMYSTIK as it stands today. For the current architecture, read
<a href="/docs/architecture/">docs/ARCHITECTURE.md</a>.</p>
</blockquote>
<p>A persistent storage peer to peer network based on <a href="https://github.com/libp2p">rust-libp2P</a>.</p>
<p><img src="/docs-media/image/mystik_p2p.png" alt="mystik_p2p"></p>
<h2 id="mechanism">Mechanism</h2>
<p>A user from a local node encrypted a file.<br></p>
<p>This encrypted file obtain a <strong>CID</strong> and is stored into local node.
At the same time the user add metadata to enable future queries from remote nodes
Metadata are name, filetype, secret code.<br></p>
<p>Each of them is actually a leaf in a merkle tree where the root is actually the <strong>MID</strong>.<br></p>
<p>In the local node then, a sparse merkle tree stores the MID / CID pair
Then, the MID is published to the network (as a DHT table: MID / Node’s address pair).</p>
<p>When a user in a remote node wants a file, it has to obtain somehow the exact name, filetype, secret code in order to let its local node computing the MID and looking for into the current DHT the MID / Node’s address pair that match the computed MID.<br>
Then the Node which has received the matching request, return the corresponding CID file.</p>
<h4 id="encryption">Encryption:<br></h4>
<p>The file is encrypted thanks to XChaCha20Poly1305 which creates file’s chunk ciphertext.<br>
The final ciphertext has the CID as name thanks to a key and nonce.</p>
<h4 id="files-metadata">File’s Metadata:<br></h4>
<p>When storing a file, user (human, AI agent) has to provide «\xA0clear text\xA0» metadata.
Each metadata is hashed and used as a leaf into a merkle tree where the roots is the MID.
As a consequence, a user (human, AI agent) which would like to call a given file, he/she/it needs to know the related «\xA0clear text\xA0» metadata.<br>
The local node will reconstruct the MID and look for into the network the nodes providers.</p>
<h4 id="sending-and-receiving-file">Sending and Receiving file:<br></h4>
<p>After an (encrypted) file has been requested, the file’s provider sends the file with a Blake hashing and the file’s requester receives the file with a Blake hashing.</p>
<h4 id="decryption">Decryption:<br></h4>
<p>To decrypt a file, one needs the key and nonce used at encryption.</p>
<pre><code>// About rust-libp2P
// Copyright 2021 Protocol Labs.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the &quot;Software&quot;),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED &quot;AS IS&quot;, WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.
</code></pre>
`},{route:`/security/`,source:`SECURITY.md`,title:`Security Policy`,html:`<h1 id="security-policy">Security Policy</h1>
<h2 id="status-pre-production-unaudited">Status: pre-production, unaudited</h2>
<p>ØMYSTIK is under active development and has <strong>not</strong> undergone independent security audit
or cryptographic review.<br>
The maintainer does not currently make a security guarantee for any release.
Treat every version as experimental.<br></p>
<p>Do not use ØMYSTIK to protect data whose disclosure would harm someone.</p>
<hr>
<h2 id="reporting-a-vulnerability">Reporting a vulnerability</h2>
<p>Report privately. <strong>Do not open a public issue for a security defect.</strong></p>
<ul>
<li>Email: <strong><a href="mailto:support@omystik.org">support@omystik.org</a></strong> with <code>[ØMYSTIK SECURITY]</code> in the subject line.</li>
<li>If you use GitHub Security Advisories, open a private advisory on the repository
instead.</li>
</ul>
<p>Please include:</p>
<ul>
<li>affected component (crate, node type, protocol) and version or commit;</li>
<li>what an attacker gains, and what access they need to start;</li>
<li>reproduction steps or a proof of concept;</li>
<li>your assessment of severity.</li>
</ul>
<p><strong>What to expect.</strong> ØMYSTIK is currently maintained by one person, so response is
best-effort rather than SLA-backed. Expect acknowledgement within about a week. If you
have had no reply in two weeks, send a follow-up before considering any disclosure.</p>
<p><strong>Disclosure.</strong> Please allow 90 days from acknowledgement before public disclosure, or
until a fix ships, whichever is sooner. If a defect is being actively exploited, say so
prominently, because that changes the handling. Reporters are credited unless they ask not to be.</p>
<p><strong>Scope.</strong> Defects in ØMYSTIK&#39;s own crates are in scope. Defects in upstream dependencies,
notably Zama&#39;s KMS and TFHE-rs and rust-libp2p, should be reported to those projects;
tell me as well if ØMYSTIK&#39;s usage makes the impact worse than upstream&#39;s own assessment.</p>
<hr>
<h2 id="known-limitations">Known limitations</h2>
<p>These are known and unresolved. They are listed so that no one deploys under a
misapprehension. Several are deliberate development conveniences that <strong>must</strong> be removed
before any production use.</p>
<h3 id="insecure-is-a-default-feature"><code>insecure</code> is a default feature</h3>
<p>The workspace manifest enables it by default:</p>
<pre><code class="language-toml">[features]
default = [&quot;p2p&quot;, &quot;insecure&quot;]
insecure = [&quot;kms/insecure&quot;]
</code></pre>
<p>This propagates Zama KMS&#39;s <code>insecure</code> feature, which enables key-generation and
decryption paths that skip protections intended for production. Documented run commands
pass <code>--features &quot;p2p insecure&quot;</code> routinely.</p>
<p><strong>Consequence:</strong> a default <code>cargo build</code> produces binaries that are not safe for
production. Any hardened build must disable it explicitly and be verified to still work.</p>
<h3 id="deterministic-node-identities-from-short-seeds">Deterministic node identities from short seeds</h3>
<p>Cluster configuration files assign each node a <code>secret_seed</code>, a single byte in the
committed examples:</p>
<pre><code class="language-toml">[[syndesmos]]
syndesmos_id = 1
secret_seed = 218
</code></pre>
<p>Node keypairs derived from a one-byte seed are trivially enumerable: an attacker who
knows the derivation can reproduce any node&#39;s private key by trying 256 values. This is
acceptable for reproducible local testing and unacceptable anywhere else.</p>
<p><strong>Consequence:</strong> every peer identity in every committed config must be treated as public.
Production deployments need real generated keys held outside the config files.</p>
<h3 id="threshold-guarantees-depend-on-party-independence">Threshold guarantees depend on party independence</h3>
<p>Threshold FHE splits trust across MPC parties (<code>num_majority</code> / <code>num_reconstruct</code> in the
gateway configuration). A cluster is sized by <strong>$n = 3t + 1$</strong>, where $t$ is the maximum
number of parties that may be compromised, so the standard 4-party cluster tolerates one.</p>
<p>Two things follow, and only the first is enforced by configuration:</p>
<ul>
<li>Exceeding $t$ compromised parties breaks the guarantee outright.</li>
<li>The guarantee holds only if no single actor controls a reconstructing subset. Party
<em>count</em> is configured; party <em>independence</em> is an operational property that nothing in
the code can check for you.</li>
</ul>
<p>Local and test clusters run all parties on one host, which provides <strong>no</strong> threshold
guarantee. It is a functional harness, not a security configuration.</p>
<h3 id="transport-and-identity">Transport and identity</h3>
<ul>
<li>The mesh runs over QUIC and nothing else, and authenticates transport with QUIC&#39;s TLS 1.3
and the Ed25519 peer identity carried in the libp2p certificate. No TCP transport is
configured, so libp2p Noise never runs. This authenticates <em>peers</em>, not the <em>humans or
agents</em> behind them.</li>
<li><strong>The MPC parties have no additional protection.</strong> The upstream KMS protects the
party-to-party link with mTLS over gRPC, and that configuration still ships in
<code>kms/core/service/config/default_*.toml</code>, but it is consumed only under
<code>#[cfg(not(feature = &quot;p2p&quot;))]</code> and the default build enables <code>p2p</code>. The certificates are
generated, loaded and ignored. Mesh B is protected exactly as Mesh A is, no more.</li>
<li><strong>DID-based access control is not implemented yet.</strong> Zero-Trust identity via W3C DIDs is
design intent recorded in the project roadmap, not shipped code. There is presently no
node-level authorisation layer beyond peer allowlists such as <code>allowed_gateway_peers</code>.</li>
<li>Rendezvous (<code>syndesmos</code>) nodes are discovery choke points. A hostile or spoofed
rendezvous node can partition or misdirect peers. Bootstrap peer IDs must be
distributed out of band and pinned.</li>
<li>No denial-of-service mitigation is implemented. This is a known open item.</li>
</ul>
<h3 id="content-and-metadata">Content and metadata</h3>
<ul>
<li>Content encryption is XChaCha20Poly1305 with a per-file key and nonce.<br>
<strong>Key custody is the caller&#39;s problem.</strong> <code>store_file</code> returns the key and nonce and the mesh does not
retain them.<br>
Lose them and the content is unrecoverable; leak them and the content is
exposed.</li>
<li>Encrypted content is hashed and provides CID.</li>
<li>Metadata privacy rests on preimage resistance.<br>
A file is addressed by a Merkle root (MID) computed from <code>file_name</code>, <code>file_type</code>, and a <code>secret</code>.
<strong>Low-entropy secrets are guessable</strong>: guess all three and you can compute the MID and find
who holds the file.</li>
<li>Local node stores the MID as key and the corresponding CID as leaf in a sparse Merkle tree.</li>
<li>MIDs are published to the DHT with the corresponding peer, so mesh membership reveals
<em>that</em> a node holds content matching a given MID. Content and metadata stay encrypted;
the fact of holding it does not.</li>
</ul>
<h3 id="cryptographic-dependencies-use-forks">Cryptographic dependencies use forks</h3>
<pre><code class="language-toml">[patch.crates-io]
attestation-doc-validation = { git = &#39;https://github.com/mkmks/attestation-doc-validation.git&#39;, ... }
rcgen        = { git = &#39;https://github.com/mkmks/rcgen.git&#39;,        branch = &#39;k256&#39; }
rustls       = { git = &#39;https://github.com/mkmks/rustls.git&#39;,       branch = &#39;k256&#39; }
rustls-pki-types = { git = &#39;https://github.com/mkmks/pki-types.git&#39;, branch = &#39;k256&#39; }
rustls-webpki    = { git = &#39;https://github.com/mkmks/webpki.git&#39;,    branch = &#39;k256&#39; }
tokio-rustls     = { git = &#39;https://github.com/mkmks/tokio-rustls.git&#39;, branch = &#39;k256&#39; }
</code></pre>
<p>These are inherited from Zama&#39;s KMS, which needs secp256k1 support in the TLS certificate
stack so a party&#39;s certificate can be signed with the same key the KMS uses for EIP-712.
They place TLS and attestation validation, which is security-critical code, on unaudited
third-party branches outside the crates.io supply chain, specified by <strong>moving branch</strong> in
the manifest rather than by immutable revision.</p>
<p><strong>Consequences.</strong> Stated precisely, because the reproducibility part is easy to overstate:</p>
<ul>
<li><code>Cargo.lock</code> does record an exact commit for each, so a build from the committed lockfile
is deterministic. What is not pinned is the <em>manifest</em>, so any <code>cargo update</code> silently
follows wherever those branches have since moved.</li>
<li>Reproducing an old build depends on a third party keeping those commits reachable. If a
repository is deleted, renamed or history-rewritten, the build stops being buildable.</li>
<li>A compromise of those repositories compromises TLS in this project.</li>
</ul>
<p>The last consequence is broader than it first appears. The patch is global, so it is not
only the KMS that gets the forked stack: <code>Cargo.lock</code> shows <code>quinn</code>, <code>quinn-proto</code> and
<code>libp2p-tls</code> all resolving to the patched <code>rustls 0.23.31</code>. Since QUIC is the only transport
the mesh uses, <strong>every handshake and every encrypted exchange between every pair of nodes is
driven by a TLS implementation living on a mutable third-party git branch.</strong> The primitives
come from <code>ring</code> and are unpatched; what the fork controls is the protocol that uses them,
including key agreement and certificate verification. Whoever controls that branch
controls the transport security of the whole mesh, and a force-push changes what your next
<code>cargo update</code> links against without changing anything you wrote.</p>
<p>Worth knowing before anyone tries to remove the patch: the secp256k1 additions are used only
on the mTLS path, which the default <code>p2p</code> build does not take. See
<a href="/docs/cryptography/#34-transport-and-mpc-certificates">CRYPTOGRAPHY.md §3.4</a>. The forked
stack is therefore carried across the whole transport for a feature the running
configuration never reaches.</p>
<p>This is not hypothetical. ØMYSTIK and the vendored KMS are separate Cargo workspaces with
separate lockfiles, and the branches moved between the two resolutions, so the lockfiles
now disagree about half of these crates:</p>
<table>
<thead>
<tr>
<th>Crate</th>
<th>root <code>Cargo.lock</code></th>
<th><code>kms/Cargo.lock</code></th>
</tr>
</thead>
<tbody><tr>
<td><code>rustls</code></td>
<td><code>2e446f8</code></td>
<td><code>71a29c7</code></td>
</tr>
<tr>
<td><code>rustls-pki-types</code></td>
<td><code>8dd57cb</code></td>
<td><code>6fe2ed5</code></td>
</tr>
<tr>
<td><code>rustls-webpki</code></td>
<td><code>9ae0560</code></td>
<td><code>5cfa16a</code></td>
</tr>
</tbody></table>
<p>The TLS stack therefore differs depending on which workspace root a build starts from, and
nothing records either resolution as deliberate. Pin exact revisions in both manifests, and
track whether the changes have been upstreamed.</p>
<h3 id="panics-on-untrusted-input">Panics on untrusted input</h3>
<p>Several code paths use <code>expect</code>/<code>assert!</code> on values derived from network input or
filesystem state. In a node process a panic is a denial of service. This has not been
systematically reviewed.</p>
<hr>
<h2 id="hardening-checklist">Hardening checklist</h2>
<p><strong>For anyone running ØMYSTIK in production</strong>, meaning a system carrying real data rather than a
local cluster, a test bench, or an evaluation deployment. Every item is controlled by
whoever operates that system, and none of it happens by default: the defaults are the
unsafe ones.</p>
<p>Before going live:</p>
<ul>
<li><input disabled="" type="checkbox"> Build with <code>--no-default-features</code> plus only the features you need; confirm
<code>insecure</code> is absent from the dependency graph.</li>
<li><input disabled="" type="checkbox"> Replace every <code>secret_seed</code> with a real generated keypair stored outside version
control.</li>
<li><input disabled="" type="checkbox"> Distribute MPC parties across independent operators and hosts; document who runs
which.</li>
<li><input disabled="" type="checkbox"> Distribute and pin rendezvous peer IDs out of band.</li>
<li><input disabled="" type="checkbox"> Use high-entropy values for the metadata <code>secret</code>.</li>
<li><input disabled="" type="checkbox"> Establish key custody for per-file encryption keys and nonces.</li>
<li><input disabled="" type="checkbox"> Confirm the <code>[patch.crates-io]</code> entries resolve to pinned revisions rather than
moving branches, and that you trust the repositories they point at.</li>
<li><input disabled="" type="checkbox"> Obtain independent security review.</li>
</ul>
<hr>
<p><em>Responsible-use scope and export-control considerations are in
<a href="/dual-use/">DUAL_USE.md</a>. Licence terms are in <a href="/licensing/">LICENSING.md</a>.</em></p>
`},{route:`/docs/architecture/`,source:`docs/ARCHITECTURE.md`,title:`ØMYSTIK Architecture`,html:`<h1 id="ømystik-architecture">ØMYSTIK Architecture</h1>
<p>This document describes how ØMYSTIK is put together: the several meshes, the gateway,
the protocols they speak, and the paths data takes through the system.</p>
<p>For what each crate contains, see <a href="/docs/crates/">CRATES.md</a>. To run it, see
<a href="/docs/getting-started/">GETTING_STARTED.md</a>.</p>
<hr>
<h2 id="1-the-principle">1. The principle</h2>
<p>In ØMYSTIK nodes find each other through rendezvous points and several meshes form a mesh over
whatever transport is available (tested so far over Wi-Fi and the internet).</p>
<p>On-demand, files stored on a certain type of nodes are encrypted before they touch
the disk and are addressed by a content identifier; the files&#39; metadata are hashed
into a Merkle root as metadata identifier.<br>
So a user who requests a file, needs to known the cleartext files&#39; metadata so that local
the mesh network propose the current owning peer(s) without anyone knowing what the content is.<br></p>
<p>Computation over data happens under fully homomorphic encryption, with the keys never assembled in one place. They are
held in threshold by a separate set of MPC parties. The result is a network that carries
data it cannot read, for peers it does not trust.</p>
<h2 id="2-three-meshes-and-a-gateway">2. Three meshes and a gateway</h2>
<p>ØMYSTIK is not one flat network. It is three meshes with distinct trust properties,
bridged deliberately rather than merged.</p>
<pre><code class="language-mermaid">graph TB
    subgraph MeshA[&quot;MESH A · transport &amp;amp; storage · /mesh-a&quot;]
        SYN[&quot;node_syndesmos&lt;br/&gt;&lt;i&gt;rendezvous / bootstrap&lt;/i&gt;&quot;]
        AUT[&quot;node_autonomos&lt;br/&gt;&lt;i&gt;peer + encrypted storage&lt;/i&gt;&quot;]
        KEN[&quot;node_kentr&lt;br/&gt;&lt;i&gt;admin / control client&lt;/i&gt;&quot;]
    end

    subgraph MeshC[&quot;MESH C · confidential compute · /mesh-c&quot;]
        YPO[&quot;node_ypolo&lt;br/&gt;&lt;i&gt;smart-program executor&lt;/i&gt;&quot;]
    end

    PYL[&quot;node_pylon&lt;br/&gt;&lt;b&gt;GATEWAY&lt;/b&gt;&lt;br/&gt;Face A ⇄ Face B&quot;]

    subgraph MeshB[&quot;MESH B · threshold FHE / KMS&quot;]
        direction TB
        subgraph KR1[&quot;node_kryphos p1&quot;]
            A1[&quot;face A&lt;br/&gt;&lt;i&gt;internal&lt;/i&gt;&quot;]
            B1[&quot;face B&lt;br/&gt;&lt;i&gt;external&lt;/i&gt;&quot;]
        end
        subgraph KR2[&quot;node_kryphos p2&quot;]
            A2[&quot;face A&lt;br/&gt;&lt;i&gt;internal&lt;/i&gt;&quot;]
            B2[&quot;face B&lt;br/&gt;&lt;i&gt;external&lt;/i&gt;&quot;]
        end
        subgraph KR3[&quot;node_kryphos p3&quot;]
            A3[&quot;face A&lt;br/&gt;&lt;i&gt;internal&lt;/i&gt;&quot;]
            B3[&quot;face B&lt;br/&gt;&lt;i&gt;external&lt;/i&gt;&quot;]
        end
        subgraph KR4[&quot;node_kryphos p4&quot;]
            A4[&quot;face A&lt;br/&gt;&lt;i&gt;internal&lt;/i&gt;&quot;]
            B4[&quot;face B&lt;br/&gt;&lt;i&gt;external&lt;/i&gt;&quot;]
        end
    end

    AUT --&gt;|Face A: jobs + blobs| PYL
    KEN --&gt;|Face A: jobs + blobs| PYL
    AUT --&gt;|computation jobs| YPO
    KEN --&gt;|computation jobs| YPO
    SYN -.-&gt;|rendezvous| AUT
    SYN -.-&gt;|rendezvous| KEN
    SYN -.-&gt;|rendezvous| YPO

    PYL ==&gt;|Face B ⇄ face B&lt;br/&gt;/mesh/gateway/crypto/1| B1
    PYL ==&gt; B2
    PYL ==&gt; B3
    PYL ==&gt; B4

    A1 &lt;--&gt;|MPC rounds&lt;br/&gt;/threshold-fhe/1| A2
    A2 &lt;--&gt; A3
    A3 &lt;--&gt; A4
    A4 &lt;--&gt; A1
    A1 &lt;--&gt; A3
    A2 &lt;--&gt; A4
</code></pre>
<h3 id="mesh-a-transport-and-encrypted-storage">Mesh A: transport and encrypted storage</h3>
<p>Namespace <code>/mesh-a</code>. This is the ØMYSTIK descendant of MYSTIK&gt;p2p: peer discovery,
persistent encrypted storage, and content routing.</p>
<p>Its swarm behaviour (<code>libp2p_common::komvos::CompositeBehaviour</code>) composes relay, ping,
identify, Kademlia DHT, rendezvous (client and server), libp2p streams for bulk transfer,
and a request-response protocol for job control.</p>
<table>
<thead>
<tr>
<th>Node</th>
<th>Binary</th>
<th>Role</th>
</tr>
</thead>
<tbody><tr>
<td><code>node_syndesmos</code></td>
<td><code>node_syndesmos</code></td>
<td>Rendezvous and bootstrap. Fixed address, well-known peer ID. Other nodes register here to be discovered.</td>
</tr>
<tr>
<td><code>node_autonomos</code></td>
<td><code>node_autonomos</code></td>
<td>The sensor node. Gathers and encrypts data, can store encrypted content to serve it to peers thanks to published Merkle roots to the DHT.</td>
</tr>
<tr>
<td><code>node_kentr</code></td>
<td><code>kentr</code></td>
<td>Control plane. Key-artifact orchestration, key rehydration, and mesh commands; carries <code>Admin</code> / <code>NonAdmin</code> roles.</td>
</tr>
</tbody></table>
<h3 id="mesh-b-threshold-fhe-and-key-management">Mesh B: threshold FHE and key management</h3>
<p>A set of MPC parties running Zama&#39;s KMS, transported over libp2p rather than gRPC. This
mesh holds the FHE key material in threshold form and performs key generation, CRS
generation, and decryption as multi-party protocols.</p>
<p><code>node_kryphos</code> (binary <code>node_fhe</code>) is one party. A cluster is typically 4 parties with
a configured majority and reconstruction threshold.
A cluster of parties follows the $3t + 1$, where $t$ is the maximal number of compromised party.</p>
<p><strong>Each party has two faces, on two separate listeners.</strong> This is the structural point about
Mesh B, and it is what keeps the key material enclosed:</p>
<table>
<thead>
<tr>
<th></th>
<th>Face A, internal</th>
<th>Face B, external</th>
</tr>
</thead>
<tbody><tr>
<td><strong>Talks to</strong></td>
<td>the other Kryphos parties</td>
<td>the Pylon gateway, and nothing else</td>
</tr>
<tr>
<td><strong>Carries</strong></td>
<td>MPC rounds: keygen, CRS gen, threshold decryption</td>
<td>crypto-operation requests and partial results</td>
</tr>
<tr>
<td><strong>Protocol</strong></td>
<td><code>mpc_protocol</code> = <code>/threshold-fhe/1</code></td>
<td><code>gateway_protocol</code> = <code>/mesh/gateway/crypto/1</code></td>
</tr>
<tr>
<td><strong>Listener</strong></td>
<td><code>listen_address</code> (e.g. <code>udp/50100</code>)</td>
<td><code>gateway_listen_addr</code> (e.g. <code>udp/8100</code>)</td>
</tr>
<tr>
<td><strong>Manager</strong></td>
<td><code>KryphosManager</code> (<code>node_kryphos::node_kryphos</code>)</td>
<td><code>KryphosPylonManager</code> (<code>node_kryphos::pylon</code>)</td>
</tr>
<tr>
<td><strong>libp2p facade</strong></td>
<td><code>libp2p_common::kryphos</code></td>
<td><code>libp2p_common::kryphos_pylon</code></td>
</tr>
</tbody></table>
<p>Both facades are thin wrappers over the same <code>libp2p_common::kryphos_dual::DualBehaviour</code>,
which registers both request-response protocols; each manager drives the one belonging to
its role.</p>
<p>So the crypto path is: <strong>Pylon&#39;s Face B connects to each party&#39;s face B.</strong> The MPC rounds
that actually do the threshold work happen entirely between the parties&#39; face A listeners.
Pylon has no part in them and never sees a round message. It issues a request on the
external faces, the parties confer among themselves internally, and each returns its partial
result on its own face B.</p>
<blockquote>
<p>A party&#39;s &quot;face A&quot; is internal to Mesh B and unrelated to Pylon&#39;s Face A, which is the
Mesh-A-facing jobs and blobs protocol. The names rhyme; the protocols do not.</p>
</blockquote>
<p><strong>This is where ØMYSTIK&#39;s principal modification to upstream Zama code lives.</strong> Zama&#39;s KMS
expects gRPC between parties; ØMYSTIK substitutes a libp2p transport (<code>kryphos_dual</code>,
<code>node_kryphos::pylon</code>) so parties can be MPC participants over the same mesh fabric as
everything else, which is what makes threshold FHE workable on links that come and go.</p>
<h3 id="mesh-c-confidential-compute">Mesh C: confidential compute</h3>
<p>Namespace <code>/mesh-c</code>. Executors that run <em><strong>smart programs</strong></em> over encrypted inputs.</p>
<p><code>node_ypolo</code> (binary <code>ypolo</code>) receives a <code>ComputeJobSpec</code>, resolves its inputs and
artifacts, runs the named program, and returns a signed receipt. It discovers peers
through Mesh A&#39;s rendezvous but registers under its own namespace, so compute capacity is
addressable separately from storage.</p>
<p>Mesh A nodes reach an executor <strong>by direct dial from configuration</strong> for simplicity, not through
discovery: a caller reads <code>peer_id</code> and <code>listen_address</code> for the target from
<code>mesh_c_cluster*.toml</code> and dials it. The Autonomos nodes push encrypted session inputs; the
Kentr nodes open the session, push its configuration, and poll for output. Both are
ordinary <code>komvos</code> clients, and the compute vocabulary rides the same Mesh A swarm.</p>
<p><code>compute-client</code> also offers <code>discover_available_ypolo</code>, which registers at a rendezvous and
probes candidates with <code>GetCapabilities</code>. It is available but currently unused: everything
in the tree dials from config instead.</p>
<p>Two executors exist: <code>executor_tfhe</code> (in-process TFHE evaluation, needs a server key) and
<code>executor_external_process</code> (spawns a separate binary implementing the program, which is how
<code>smart-program-alert</code> is deployed).</p>
<h3 id="the-pylon-gateway">The Pylon gateway</h3>
<p><code>node_pylon</code> is the bridge, and it is deliberately the only one. It has two faces:</p>
<ul>
<li><strong>Face A</strong> looks at Mesh A: job control and blob transfer with ordinary nodes.</li>
<li><strong>Face B</strong> looks at Mesh B: crypto operations against the MPC parties, fanned out to all
parties and aggregated according to <code>num_majority</code> / <code>num_reconstruct</code>.</li>
</ul>
<p>Keeping the crypto mesh reachable only through a gateway means Mesh A peers never hold a
direct channel to an MPC party. A hostile storage node cannot address the key material.</p>
<p>To avoid critical bottleneck, a dedicated mechanism to manage several Pylon nodes is under development.</p>
<h3 id="how-nodes-find-each-other">How nodes find each other</h3>
<p>Five distinct mechanisms, each used for a different kind of target. Which one applies
depends on what is being looked for, and they are easy to confuse.</p>
<p><strong>1 · Rendezvous registration: how a node joins.</strong>
Every node registers with a Syndesmos node under a namespace: <code>/mesh-a</code> for Autonomos and
Kentr, <code>/mesh-c</code> for Ypolo.</p>
<pre><code class="language-rust">manager.register_rendezvous(syndesmos_addr, syndesmos_peer, namespace)
</code></pre>
<p>The rendezvous point itself is <strong>not discovered</strong>. Its <code>peer_id</code> and address are pinned in
the cluster TOML (<code>syndesmos_peer</code> / <code>syndesmos_addr</code>, <code>syndesmos_peer_id_1</code> /
<code>syndesmos_address</code>). This is the trust anchor: a node believes the mesh it is told to
believe. Registration makes a node visible to others and connects it to the swarm; it is not
a lookup.</p>
<p><strong>2 · Identify: how the Pylon gateway is found.</strong>
Nobody configures the gateway&#39;s address. <code>KomvosManager</code> watches
<code>komvos::Event::PeerIdentified</code> and records any peer whose libp2p identify
<code>protocol_version</code> is <code>/pylon-face-a/0.1.0</code>:</p>
<pre><code class="language-rust">fn is_pylon_face_a_protocol(protocol_version: &amp;str) -&gt; bool {
    protocol_version == &quot;/pylon-face-a/0.1.0&quot;
}
</code></pre>
<p>Those peers accumulate in <code>pylon_face_a_peers</code>, and <code>manager.pylon_peers()</code> returns them.
A Kentr node that finds the list empty cannot perform a threshold decryption yet. It must
wait for a Pylon to appear. So the gateway is discovered <em>by protocol version</em>, structurally
rather than by name.</p>
<p><strong>3 · Kademlia DHT provider records: how stored content is located.</strong>
The only mechanism that is a genuine lookup by name. A node advertises what it holds with
<code>spread_into_network</code> (a <code>start_providing</code> call) and a consumer resolves it with
<code>get_providers</code>; both map onto <code>kademlia.start_providing</code> / <code>kademlia.get_providers</code> over
the name&#39;s bytes.</p>
<p>This is how <strong>content</strong> is found: providers (the value) are advertised under the file&#39;s MID (the key), and a peer
that has recomputed the MID from the metadata resolves the providers holding it.
<br>
Then, file&#39;s owner needs to find the CID (Content ID) into a local merkle tree. <a href="#4-how-file-storage-and-exchange-works">See §4</a>.</p>
<p>The DHT holds provider records. No file is stored in it.</p>
<p><strong>4 · Face A <code>GetArtifact</code>: how FHE key material enters Mesh A.</strong>
Key artifacts are generated by the MPC then each party owns the data.<br>
Currently, they are held by the Pylon, and pulled from it by Mesh A&#39;s nodes over Face A:</p>
<ul>
<li>Kentr: all type of keys,</li>
<li>Autonomos: PublicKey,</li>
<li>Ypolo: ServerKey.</li>
</ul>
<p><code>node_kentr::mesh_cmd::fetch_keyset_artifacts_from_pylon</code> requests each kind explicitly:</p>
<pre><code class="language-rust">net_client.jobs_request(
    pylon_peer,
    FaceARequest::GetArtifact { id: key_id, kind },   // PublicKey
).await                                              // PublicKeyMetadata
                                                     // ServerKey
</code></pre>
<p>The response carries the bytes as <code>Payload::Inline</code>, or as a <code>Payload::BlobRef</code> then fetched
with <code>get_blob</code> over the Face A blob protocol, since a server key is far too large to inline.
This is request-response against one already-known gateway: no lookup, no provider record,
no Kademlia.</p>
<p>Distribution therefore happens in <strong>two stages</strong>, and conflating them is the easy mistake:</p>
<table>
<thead>
<tr>
<th></th>
<th>Stage 1, acquisition</th>
<th>Stage 2, replication</th>
</tr>
</thead>
<tbody><tr>
<td><strong>From</strong></td>
<td>the Pylon (holding Mesh B&#39;s keys)</td>
<td>another Mesh A peer that already has a copy</td>
</tr>
<tr>
<td><strong>By</strong></td>
<td>a Kentr node, <code>--rehydrate-key</code></td>
<td>any node calling <code>ask_key</code></td>
</tr>
<tr>
<td><strong>Mechanism</strong></td>
<td>Face A <code>GetArtifact</code> (+ <code>get_blob</code>)</td>
<td><code>/key/1</code> stream</td>
</tr>
<tr>
<td><strong>Role of the DHT</strong></td>
<td>none</td>
<td>locating <em>which peer</em> holds a copy</td>
</tr>
</tbody></table>
<p>Stage 2 is what <code>KeyServices::get_key</code> performs: <code>get_providers(key_type)</code> resolves peers
that advertised the key-type <strong>name</strong> (<code>PublicKey</code>, <code>PublicKeyMetadata</code>, <code>ServerKey</code>), then
it dials one and pulls the bytes over <code>/key/1</code>. A node becomes such a provider only after
holding a copy. <code>key_rehydration</code> saves the artifact with <code>save_key_at_path</code> and <em>then</em>
calls <code>spread_into_network(key_type.to_string())</code>. Each node that later acquires a copy
advertises itself in turn, so copies fan out across Mesh A while every original still comes
from the gateway.</p>
<p><code>ask_key</code> wraps stage 2 in a retry loop that first wipes the local key directory and does not
return until a key arrives and validates. Key acquisition is treated as mandatory: a node
that cannot obtain the public key cannot encrypt, so it waits rather than continuing.</p>
<p><strong>5 · Direct dial from configuration: how executors and MPC parties are reached.</strong><br>
Mesh C executors and Mesh B parties are addressed from config, not looked up. A caller
takes the target&#39;s <code>peer_id</code> and <code>listen_address</code> straight from <code>mesh_c_cluster*.toml</code> and
dials it; Kryphos party addresses come from <code>mpc_cluster*.toml</code>, held only by Pylon. This is
the least dynamic mechanism and the one most in tension with the project&#39;s resilience goals
A rendezvous-based path exists (<code>discover_available_ypolo</code>) but nothing in the tree uses
it yet.</p>
<table>
<thead>
<tr>
<th>Target</th>
<th>Mechanism</th>
<th>Source of truth</th>
</tr>
</thead>
<tbody><tr>
<td>Syndesmos (rendezvous)</td>
<td>pinned</td>
<td>cluster TOML</td>
</tr>
<tr>
<td>Peers in a namespace</td>
<td>rendezvous registration</td>
<td>Syndesmos</td>
</tr>
<tr>
<td>Pylon gateway</td>
<td>libp2p identify, protocol-version match</td>
<td><code>/pylon-face-a/0.1.0</code></td>
</tr>
<tr>
<td>Content (by MID)</td>
<td>Kademlia provider records, then <code>/file-exchange/1</code></td>
<td>DHT</td>
</tr>
<tr>
<td>FHE key artifacts, original</td>
<td>Face A <code>GetArtifact</code> / <code>get_blob</code></td>
<td>the Pylon</td>
</tr>
<tr>
<td>FHE key artifacts, replica</td>
<td>Kademlia name index, then <code>/key/1</code></td>
<td>a peer that already holds one</td>
</tr>
<tr>
<td>Ypolo executor</td>
<td>direct dial</td>
<td><code>mesh_c_cluster*.toml</code></td>
</tr>
<tr>
<td>Kryphos parties</td>
<td>direct dial (Pylon only)</td>
<td><code>mpc_cluster*.toml</code></td>
</tr>
</tbody></table>
<h2 id="3-protocol-identifiers">3. Protocol identifiers</h2>
<table>
<thead>
<tr>
<th>Constant</th>
<th>Value</th>
<th>Between</th>
</tr>
</thead>
<tbody><tr>
<td><code>FILE_EX_PROTOCOL</code></td>
<td><code>/file-exchange/1</code></td>
<td>Mesh A peers, bulk file transfer over streams</td>
</tr>
<tr>
<td><code>KEY_RAW_PROTOCOL</code></td>
<td><code>/key/1</code></td>
<td>Mesh A peers, public-key / server-key artifact transfer</td>
</tr>
<tr>
<td><code>NAMESPACE_MESH_A</code></td>
<td><code>/mesh-a</code></td>
<td>Mesh A rendezvous namespace</td>
</tr>
<tr>
<td><code>FACE_A_JOBS_PROTOCOL</code></td>
<td><code>/mesh/gateway/jobs/1</code></td>
<td>Mesh A node → Pylon <em>and</em> Mesh A node → Ypolo</td>
</tr>
<tr>
<td><code>FACE_A_BLOBS_PROTOCOL</code></td>
<td><code>/mesh/gateway/blob/1</code></td>
<td>Blob upload and fetch, against Pylon or Ypolo</td>
</tr>
<tr>
<td><code>FACE_B_CRYPTO_PROTOCOL</code></td>
<td><code>/mesh/gateway/crypto/1</code></td>
<td>Pylon&#39;s Face B → each Kryphos party&#39;s face B</td>
</tr>
<tr>
<td><code>mpc_protocol</code></td>
<td><code>/threshold-fhe/1</code></td>
<td>Kryphos face A ⇄ Kryphos face A, MPC rounds internal to Mesh B</td>
</tr>
<tr>
<td>n/a</td>
<td><code>/mesh-c</code></td>
<td>Mesh C rendezvous namespace</td>
</tr>
</tbody></table>
<p>Face A is served by both Pylon and Ypolo. The same request-response vocabulary asks a
gateway for a crypto operation and an executor for a computation; where discovery is used,
a caller tells them apart with <code>GetCapabilities</code> and the returned <code>NodeType</code>. What differs
is how each is reached: the gateway by identify, the executor by direct dial.</p>
<p>Transport throughout is QUIC (<code>/udp/&lt;port&gt;/quic-v1</code>), and only QUIC. Every swarm is built
<code>.with_tokio().with_quic()</code>, no TCP leg is configured, and peer authentication comes from
QUIC&#39;s TLS 1.3 with the node&#39;s Ed25519 identity bound in through the libp2p certificate
extension. Noise plays no part, despite being the libp2p default most readers expect. See
<a href="/docs/cryptography/#21-own-crates">CRYPTOGRAPHY.md §2.1</a>.</p>
<h2 id="4-how-file-storage-and-exchange-works">4. How file storage and exchange works</h2>
<p>The storage and exchange model is inherited from MYSTIK&gt;p2p, and its two-level Merkle structure is the
part worth understanding. The original description of that model, written for the earlier
project, is kept at <a href="/readme-mystik-p2p/">README_mystik-p2p.md</a>.</p>
<pre><code class="language-mermaid">graph LR
    F[&quot;plaintext file&quot;] --&gt;|XChaCha20Poly1305&lt;br/&gt;streaming, 2 MiB chunks| C[&quot;ciphertext&lt;br/&gt;named by &lt;b&gt;CID&lt;/b&gt;&quot;]
    F -.-&gt;|user supplies| M[&quot;metadata:&lt;br/&gt;file_name · file_type · secret&quot;]
    M --&gt;|hash each as a leaf&lt;br/&gt;&lt;i&gt;rs_merkle&lt;/i&gt;| MID[&quot;&lt;b&gt;MID&lt;/b&gt;&lt;br/&gt;Merkle root&quot;]
    MID --&gt;|key| SMT[&quot;sparse Merkle tree&lt;br/&gt;&lt;i&gt;monotree + blake3&lt;/i&gt;&quot;]
    C --&gt;|leaf| SMT
    MID --&gt;|published| DHT[(&quot;Kademlia DHT&lt;br/&gt;MID → provider(s)&quot;)]
</code></pre>
<p><strong>Storing file.</strong><br>
<code>persistency_utils::store_file</code> generates a fresh 32-byte key and 19-byte
nonce, encrypts the file in 2 MiB chunks with XChaCha20Poly1305, and names the ciphertext
by its content identifier (CID). <br>
Separately, the caller&#39;s three metadata fields
(<code>file_name</code>, <code>file_type</code>, <code>secret</code>) become leaves of a classic Merkle tree whose root is
the metadata identifier (MID). <br>
A sparse Merkle tree then records the MID → CID pair, and
the MID is published to the Kademlia DHT as a provider record.</p>
<p>The function returns the key, the nonce, the MID, and the CID.
<strong>The mesh keeps no copy of the key or nonce.</strong></p>
<p><strong>Retrieving file.</strong><br>
A user who knows all three metadata values asks a local or remote peer, which recomputes the
MID, queries the DHT for providers, and requests the file over <code>/file-exchange/1</code>.<br></p>
<p><strong>Transfers</strong> are Blake3-hashed on both ends.<br></p>
<p><strong>Decrypting file.</strong><br>
requires the key and nonce, which must have reached the requester by some other channel.</p>
<p>The design consequence: <strong>knowledge of the metadata is the capability to locate content</strong>,
in those two steps. The <code>secret</code> field is the access control. See
<a href="/security/#content-and-metadata">SECURITY.md</a> on why low-entropy secrets defeat this.</p>
<h2 id="5-how-a-confidential-computation-works">5. How a confidential computation works</h2>
<p>The OSATCON demo is a perfect fit to explain the mechanism.</p>
<p>Encrypting data, computing over it, and decrypting the result each cross a different
trust boundary.</p>
<pre><code class="language-mermaid">sequenceDiagram
    participant A as Application&lt;br/&gt;(osatcon operator)
    participant C as Crypto client&lt;br/&gt;(pelatis via kentr)
    participant P as Pylon&lt;br/&gt;(gateway)
    participant K as Kryphos parties&lt;br/&gt;(face B → face A)
    participant Y as Ypolo&lt;br/&gt;(smart program / fhe computation)

    Note over C,K: key material
    C-&gt;&gt;P: Face A, request keyset artifacts
    P-&gt;&gt;K: Face B → party face B: CRS gen / keygen preproc / keygen
    K--&gt;&gt;K: MPC rounds between face A listeners&lt;br/&gt;(/threshold-fhe/1)
    K--&gt;&gt;P: on each party&#39;s face B:&lt;br/&gt;public key · server key (private held in threshold)
    P--&gt;&gt;C: GetArtifact: bytes inline, or BlobRef + get_blob

    Note over A,Y: computation
    A-&gt;&gt;A: encrypt inputs with public key (TFHE compact list)
    A-&gt;&gt;Y: Face A, put_blob(ciphertext)
    A-&gt;&gt;Y: Face A, submit ComputeJobSpec(program_id, inputs, artifacts)
    Y-&gt;&gt;Y: execute smart program over ciphertext
    Y--&gt;&gt;A: ComputeReceipt + encrypted outputs

    Note over C,K: decryption
    A-&gt;&gt;C: hand off ciphertext to decrypt
    C-&gt;&gt;P: Face A, public-decrypt / user-decrypt request
    P-&gt;&gt;K: Face B → party face B: fan out to all parties
    K--&gt;&gt;K: threshold decryption rounds on face A
    K--&gt;&gt;P: partial results, each on its own face B
    P--&gt;&gt;C: aggregated plaintext (num_reconstruct satisfied)
</code></pre>
<p>Two distinct callers appear here.</p>
<ul>
<li>The <strong>application</strong> (an OSATCON operator embedding
<code>smart-program-alert</code>) owns the compute path to Ypolo.</li>
<li>The <strong>crypto client</strong>
(<code>plegma-fhe-pelatis</code>, driven through <code>node_kentr</code>) owns the key and decryption path to
Pylon.<br>
In \`headquarter\` both live in one process, which is why the split is easy to miss.</li>
</ul>
<p>Three properties fall out of this shape:</p>
<ol>
<li><strong>Ypolo never holds a decryption key.</strong> It computes on ciphertext with a server key
(evaluation key) and cannot read its own inputs or outputs.</li>
<li><strong>No single Kryphos party can decrypt.</strong> Reconstruction needs <code>num_reconstruct</code>
parties to cooperate.</li>
<li><strong>Pylon aggregates but does not decrypt.</strong> It collects partials from the parties&#39;
external faces and applies the threshold rule; it holds no share of its own, and the
rounds that combine the shares happen on the internal faces where it has no presence.</li>
</ol>
<p>A <em>smart program</em> is a unit of computation with a stable identity (<code>ProgramId</code> +
<code>ProgramVersion</code>), a manifest declaring its slots and artifacts, and a typed ABI.<br>
The <code>SmartProgram</code> trait in <code>compute-program</code> is what a domain crate implements; <code>compute-abi</code>
defines the wire types both sides agree on. Programs can be session-oriented, accepting
live inputs over time via <code>try_build_session_inputs</code> and running when they have enough,
which is how the OSATCON alert program consumes a stream of positions.</p>
<h2 id="6-reference-application-osatcon">6. Reference application: OSATCON</h2>
<p><code>osatcon/</code> is a working end-to-end demonstration, and the clearest illustration of what
the mesh is for.<br>
The scenario: a convoy moving along a ground corridor, satellites
overhead, and a question: <em>is the convoy about to be observed?</em></p>
<p>Answering it normally means someone pools the convoy&#39;s route with the satellite ephemeris.
Under ØMYSTIK, nobody does. Convoy positions and satellite tracks are encrypted by their
own operators; the exposure computation runs homomorphically on Ypolo; only the alert is
decrypted (here by headquarter).</p>
<table>
<thead>
<tr>
<th>Crate</th>
<th>Role</th>
</tr>
</thead>
<tbody><tr>
<td><code>ofield-core</code></td>
<td>Domain maths: geodesy, projection, convoy interpolation, satellite tracks, alert geometry, and a versioned scenario format (<code>OsatconScenarioV1</code>) so every participant computes against identical parameters</td>
</tr>
<tr>
<td><code>smart-program-alert</code></td>
<td>The FHE smart program <code>satcon.alert_math</code> v0.1.0, computing exposure over <code>FheUint64</code>/<code>FheBool</code>, plus <code>deploy_alert</code>, which installs it on an Ypolo executor</td>
</tr>
<tr>
<td><code>edge-convoy-operator</code></td>
<td>Field terminal for the convoy; contributes encrypted positions</td>
</tr>
<tr>
<td><code>edge-satellite-operator</code></td>
<td>Field terminal for satellite tracking; contributes encrypted tracks</td>
</tr>
<tr>
<td><code>headquarter</code></td>
<td>Command view; requests decryption of alerts through Kentr</td>
</tr>
</tbody></table>
<h3 id="deployment-topology">Deployment topology</h3>
<p>Each OSATCON binary <em>is</em> a Mesh A node: it builds a <code>KomvosManager</code> from a node
configuration and takes that node&#39;s identity. Two run as Autonomos, two as Kentr. Mesh B is
omitted here; it sits behind the Pylon exactly as in <a href="#2-three-meshes-and-a-gateway">§2</a>.</p>
<pre><code class="language-mermaid">graph TB
    subgraph MeshA[&quot;MESH A · /mesh-a&quot;]
        SYN[&quot;node_syndesmos&lt;br/&gt;&lt;i&gt;rendezvous&lt;/i&gt;&quot;]
        CONV[&quot;&lt;b&gt;edge-convoy-operator&lt;/b&gt;&lt;br/&gt;node_autonomos id=1&quot;]
        SAT[&quot;&lt;b&gt;edge-satellite-operator&lt;/b&gt;&lt;br/&gt;node_autonomos id=2&quot;]
        HQ[&quot;&lt;b&gt;headquarter&lt;/b&gt; · &lt;b&gt;deploy_alert&lt;/b&gt;&lt;br/&gt;node_kentr id=2 · NonAdmin&quot;]
        ADM[&quot;&lt;i&gt;key management&lt;/i&gt;&lt;br/&gt;node_kentr id=1 · Admin&quot;]
    end

    subgraph MeshC[&quot;MESH C · /mesh-c&quot;]
        YPO[&quot;node_ypolo&lt;br/&gt;&lt;i&gt;runs&lt;/i&gt; satcon.alert_math&quot;]
    end

    PYL[&quot;node_pylon&lt;br/&gt;&lt;b&gt;GATEWAY&lt;/b&gt;&quot;]

    SYN -.-&gt;|register| CONV
    SYN -.-&gt;|register| SAT
    SYN -.-&gt;|register| HQ
    SYN -.-&gt;|register| ADM
    SYN -.-&gt;|register| YPO

    CONV --&gt;|&quot;push_convoy_payload_v1&lt;br/&gt;&lt;i&gt;encrypted x,y&lt;/i&gt;&quot;| YPO
    SAT --&gt;|&quot;push_satellite_payload_v1&lt;br/&gt;&lt;i&gt;encrypted track&lt;/i&gt;&quot;| YPO
    HQ --&gt;|&quot;start_alert_session_v1&lt;br/&gt;push_alert_config_v1&lt;br/&gt;poll_alert_output_v1&quot;| YPO
    HQ --&gt;|&quot;deploy_alert:&lt;br/&gt;install program&quot;| YPO

    HQ -.-&gt;|&quot;pylon_peers()&lt;br/&gt;&lt;i&gt;via identify&lt;/i&gt;&quot;| PYL
    HQ --&gt;|&quot;threshold decrypt&lt;br/&gt;of alert output&quot;| PYL
    ADM --&gt;|&quot;GetArtifact:&lt;br/&gt;PublicKey · ServerKey&lt;br/&gt;&lt;i&gt;rehydrate&lt;/i&gt;&quot;| PYL

    ADM -.-&gt;|&quot;/key/1 replica&lt;br/&gt;&lt;i&gt;provider found by name&lt;/i&gt;&quot;| CONV
    ADM -.-&gt;|&quot;/key/1 replica&quot;| SAT
</code></pre>
<p>The sequence in practice:</p>
<ol>
<li><strong>Admin Kentr (id 1)</strong> generates or rehydrates the keyset by pulling it from the Pylon
with Face A <code>GetArtifact</code>, saves it locally, and only then advertises itself as a
provider of those key names, after which Mesh A peers can replicate copies from it over
<code>/key/1</code>.</li>
<li><strong><code>deploy_alert</code></strong>, running as Kentr id 2, installs <code>satcon.alert_math</code> on the executor.
Its <code>--runner-command</code> is the path to the <code>smart-program-alert</code> binary <em>as seen from the
Ypolo host</em>, because the program runs as an external process there.</li>
<li><strong>Headquarter</strong> registers, discovers the Pylon by identify, builds a <code>ThresholdFheClient</code>
over the discovered gateways, dials Ypolo, opens the session, and pushes the encrypted
alert-zone configuration (the radius from the scenario).</li>
<li><strong>The two edge operators</strong> each fetch the public key with <code>ask_key</code>, dial Ypolo, and then
loop: sample position or track, project it to integers, encrypt each coordinate as
<code>FHE_UINT64</code>, and push it as a session input.</li>
<li><strong>Ypolo</strong> evaluates the alert program over the accumulated ciphertexts as inputs arrive.</li>
<li><strong>Headquarter</strong> polls the encrypted output and decrypts it through the threshold, the
only point in the whole flow where plaintext appears.</li>
</ol>
<p>Everything binding the participants together is the <strong>session id</strong>, derived from a shared
<code>--session-code</code> (<code>session_id_from_code</code>). All four must be started with the same code, and
all read the same <code>OsatconScenarioV1</code> so their projections and tick alignment agree.
Otherwise they encrypt coordinates that are not comparable.</p>
<p>This is the builds dispatched:</p>
<ul>
<li>2 * Raspberry Pi Zero 2 W: syndesmos, pylon (--target armv7-unknown-linux-gnueabihf),</li>
<li>1 * Raspberry Pi 5: edge-convoy-operator (--target aarch64-unknown-linux-gnu),</li>
<li>1 * 2011 MacBook running Kali Linux: edge-satellite-operator (--target x86_64-unknown-linux-gnu),</li>
<li>1 * 2013 MacBook Pro: ypolo / smart-program-alert (--target x86_64-apple-darwin)</li>
<li>1 * 2018 MacBook Pro: kentr/headquarter (--target x86_64-unknown-linux-gnu)</li>
<li>4 * Computation Unit (Google Cloud Platorm): kryphos (--target x86_64-unknown-linux-gnu),</li>
</ul>
<h2 id="7-design-decisions-worth-knowing">7. Design decisions worth knowing</h2>
<p><strong>Several meshes rather than one</strong> Trust separation and different behaviors for different models.</p>
<p><strong>Why vendor Zama&#39;s KMS instead of depending on it?</strong> The libp2p transport substitution is
invasive, since it replaces the inter-party communication layer. See
<a href="/licensing/#2-zama-components-commercial-use-requires-an-agreement">LICENSING.md §2</a>
for the obligations vendoring creates, and note that upstream fixes must be ported by hand.</p>
<p><strong>QUIC only</strong><br>
Faster recovery from path changes than TCP, chosen with roaming nodes in mind: a terminal
that changes address mid-session should keep the session rather than rebuild it. This
release has only been run over Wi-Fi and wired internet, so that is the reason for the
choice and not a result measured on this system.</p>
<hr>
<p><em>Security posture: <a href="/security/">SECURITY.md</a> · Responsible use:
<a href="/dual-use/">DUAL_USE.md</a> · Crate reference: <a href="/docs/crates/">CRATES.md</a></em></p>
`},{route:`/docs/crates/`,source:`docs/CRATES.md`,title:`Crate Reference`,html:`<h1 id="crate-reference">Crate Reference</h1>
<p>The workspace has 24 members plus the vendored <code>kms/</code> tree and the root <code>mesh</code> crate. This
document groups them by layer and states what each one owns.</p>
<p>For how they fit together, read <a href="/docs/architecture/">ARCHITECTURE.md</a> first.</p>
<blockquote>
<p><strong>Not crates:</strong> <code>nodeA/</code> … <code>nodeN/</code> are per-node <em>runtime data trees</em>, not Rust packages.
See <a href="/docs/node-layout/">NODE_LAYOUT.md</a>.</p>
</blockquote>
<hr>
<h2 id="dependency-shape">Dependency shape</h2>
<p>To provide a global understanding, this dependency shape shows the whole OSATCON demo.<br></p>
<pre><code class="language-mermaid">---
title: &quot;Crate dependencies: each arrow runs from a crate to something it needs&quot;
---
graph TD
    subgraph LEGEND[&quot;how to read this&quot;]
        direction LR
        EXA[&quot;a crate&quot;] --&gt;|needs| EXB[&quot;its dependency&quot;]
    end

    subgraph OSAT[&quot;OSATCON demo&quot;]
        CONV[&quot;edge-convoy-operator&quot;]
        SAT[&quot;edge-satellite-operator&quot;]
        HQ[&quot;headquarter&quot;]
        SPA[&quot;smart-program-alert&quot;]
        OFC[&quot;ofield-core&quot;]
    end

    subgraph NODES[&quot;nodes&quot;]
        SYN[&quot;node_syndesmos&quot;]
        AUT[&quot;node_autonomos&quot;]
        KEN[&quot;node_kentr&quot;]
        YPO[&quot;node_ypolo&quot;]
        PYL[&quot;node_pylon&quot;]
        KRY[&quot;node_kryphos&quot;]
    end

    subgraph LIBS[&quot;libraries&quot;]
        PELATIS[&quot;plegma-fhe-pelatis&quot;]
        CCLI[&quot;compute-client&quot;]
        KRYPT[&quot;kryptografia&quot;]
        L2P[&quot;libp2p_common&quot;]
        MGW[&quot;mesh_gateway_wire&quot;]
        CPROG[&quot;compute-program&quot;]
        CCORE[&quot;compute-core&quot;]
        PERS[&quot;persistency_utils&quot;]
        DBA[&quot;db-access&quot;]
        CABI[&quot;compute-abi&quot;]
        CH[&quot;content-hashing&quot;]
        MRKL[&quot;metadata-mrkl&quot;]
        MCFG[&quot;meshes_config&quot;]
    end

    KMS[(&quot;kms/ · vendored Zama&quot;)]

    CONV --&gt; AUT &amp; SPA &amp; KRYPT &amp; OFC
    SAT --&gt; AUT &amp; SPA &amp; KRYPT &amp; OFC
    HQ --&gt; KEN &amp; PELATIS &amp; SPA &amp; KRYPT &amp; OFC
    SPA --&gt; KEN &amp; CCLI &amp; CPROG

    KEN --&gt; PELATIS &amp; L2P &amp; MGW &amp; KMS
    YPO --&gt; CPROG &amp; L2P &amp; MGW
    AUT --&gt; L2P &amp; PERS &amp; KRYPT
    SYN --&gt; L2P &amp; PERS
    PYL --&gt; L2P &amp; MGW &amp; MCFG
    KRY --&gt; L2P &amp; MGW &amp; KMS

    PELATIS --&gt; L2P &amp; MGW &amp; KMS
    CCLI --&gt; L2P &amp; MGW &amp; CABI
    KRYPT --&gt; KMS
    L2P --&gt; MGW &amp; MCFG &amp; CH &amp; DBA
    MGW --&gt; CABI &amp; KMS
    CPROG --&gt; CCORE --&gt; CABI
    PERS --&gt; CH &amp; MRKL
    DBA --&gt; CH &amp; MRKL
</code></pre>
<hr>
<h2 id="storage-and-cryptography">Storage and cryptography</h2>
<h3 id="content-hashing"><code>content-hashing</code></h3>
<blockquote>
<p><em>Encryption content, Hashing checking</em></p>
</blockquote>
<p>Content encryption and content addressing. <code>newkey()</code> generates a 32-byte key and 19-byte
nonce; <code>encrypt_file</code> / <code>decrypt_file</code> stream files through <strong>XChaCha20Poly1305</strong> in 2 MiB
chunks (<code>EncryptorBE32</code> / <code>DecryptorBE32</code>), producing a ciphertext named by its <strong>CID</strong>.
The <code>blakecheck</code> module provides Blake3 verification for transfers.</p>
<p>Storage locations come from the environment (<code>STORAGE_DIR</code>, <code>KEYS_DIR</code>) with defaults
under <code>./data/</code>.</p>
<p><strong>Owns the invariant that plaintext never reaches disk.</strong></p>
<h3 id="metadata-mrkl"><code>metadata-mrkl</code></h3>
<blockquote>
<p><em>Merkle Tree applied to metadata</em></p>
</blockquote>
<p>Two Merkle layers over file metadata:</p>
<ol>
<li><strong>Classic tree</strong> (<code>rs_merkle</code>, SHA-256) hashes <code>file_name</code>, <code>file_type</code>, <code>secret</code> as
leaves; the root is the <strong>MID</strong> (metadata identification). <code>prove_leaf_persistent</code> proves a leaf&#39;s membership.</li>
<li><strong>Sparse tree</strong> (<code>monotree</code> + Blake3 + <code>sled</code>): <code>MonoMetaTree</code> maps MID → CID
persistently (content identification).</li>
</ol>
<p>Reads <code>STORAGE_MERKLE</code> and <code>STORAGE_MIDCID</code> from the environment. <code>store_rsmerkle</code>
(private) handles persistence of the classic trees.</p>
<h3 id="persistency_utils"><code>persistency_utils</code></h3>
<p>Orchestration over the two above. <code>store_file(file_name, file_type, secret, file_path)</code>
runs the whole store path (key generation, encryption, MID computation, membership proof,
sparse-tree insertion) and returns <code>(key, nonce, MID, CID)</code>. Companion retrieval and
removal paths mirror it.</p>
<p><strong>This is the crate to read first</strong> if you want to understand the storage model, because it
is where the sequence is visible in one place. For the model stated in prose rather than
code, see <a href="/readme-mystik-p2p/">README_mystik-p2p.md</a>.</p>
<h3 id="db-access"><code>db-access</code></h3>
<p>Reconstructs MID/CID pairs by walking the filesystem rather than consulting an index. CIDs
are <code>.bin</code> files under the content directory, MIDs are subdirectory names under
<code>metadata/metamerkle</code>. Recovery and inspection utility; the authoritative mapping lives in
the sparse Merkle tree.<br>
Useful when a node starts with existing stored content: that inventory has to be fed back
into the network before peers can find it again.</p>
<h3 id="kryptografia"><code>kryptografia</code></h3>
<p>The FHE encryption boundary. <code>FheEncryptor</code> wraps a TFHE <code>CompactPublicKey</code> and builds
<code>CompactCiphertextList</code>s using TFHE safe serialization (<code>tfhe.safe_serialize.CompactCiphertextList.v1</code>).<br>
Bridges ØMYSTIK types to KMS ciphertext types (<code>TypedCiphertext</code>, <code>TypedPlaintext</code>, <code>CiphertextFormat</code>)
and carries the FHE type constants, including <code>FHE_INT64 = 10064</code>, a local extension
because the KMS <code>FheType</code> enum has no signed types.</p>
<hr>
<h2 id="networking-and-wire-protocol">Networking and wire protocol</h2>
<h3 id="libp2p_common"><code>libp2p_common</code></h3>
<blockquote>
<p><em>libp2p for FHE p2p mesh</em></p>
</blockquote>
<p>Every swarm behaviour in the project. The largest and most central library, though not the
lowest: it depends on <code>mesh_gateway_wire</code>, <code>meshes_config</code>, <code>content-hashing</code> and
<code>db-access</code>, so the wire types and storage primitives sit beneath it.</p>
<table>
<thead>
<tr>
<th>Module</th>
<th>Lines</th>
<th>Contents</th>
</tr>
</thead>
<tbody><tr>
<td><code>komvos</code></td>
<td>1733</td>
<td><strong>Mesh A.</strong> <code>CompositeBehaviour</code>: relay, ping, identify, Kademlia, rendezvous (client + server), streams, request-response. The <code>Client</code> / <code>EventLoop</code> / <code>Event</code> triple: <code>start_listening</code>, <code>advertise</code>, <code>register</code>, <code>dial</code>, <code>start_providing</code>, <code>get_providers</code>, <code>request_file</code>, <code>request_key</code>, <code>get_blob</code>, <code>put_blob</code>, <code>jobs_request</code>, <code>ack_pinned</code></td>
</tr>
<tr>
<td><code>kryphos_dual</code></td>
<td>841</td>
<td><strong>Mesh B.</strong> <code>DualBehaviour</code>: two request-response protocols: <code>ThresholdCodec</code> for MPC rounds and <code>ComputeEncCodec</code> for the gateway-facing face. <code>parties_mesh_ready</code> gates on party connectivity</td>
</tr>
<tr>
<td><code>kryphos</code></td>
<td>62</td>
<td>Thin facade over <code>kryphos_dual</code> driving a party&#39;s <strong>internal face A</strong> (MPC)</td>
</tr>
<tr>
<td><code>kryphos_pylon</code></td>
<td>78</td>
<td>Thin facade over <code>kryphos_dual</code> driving a party&#39;s <strong>external face B</strong> (kryphos&#39; gateway)</td>
</tr>
<tr>
<td><code>manager</code></td>
<td>338</td>
<td><code>KomvosManager</code>, node lifecycle: construction, inbound request task, rendezvous registration, MID/CID registration, Pylon peer lookup. Vends <code>KeyServices</code> and <code>FileServices</code></td>
</tr>
<tr>
<td><code>node_services</code></td>
<td>302</td>
<td><code>KeyServices</code> (<code>get_key</code>, <code>ask_key</code>) and <code>FileServices</code> (<code>init_publishing_mid_to_dht</code>, <code>get_file</code>, <code>access_the_db</code>), plus <code>spread_into_network</code></td>
</tr>
<tr>
<td><code>node_primary</code></td>
<td>241</td>
<td><code>NodeType</code>, <code>NodeRole</code>, <code>KeyType</code>; protocol constants; per-node-type config structs deserialized from TOML</td>
</tr>
<tr>
<td><code>jobscodec</code></td>
<td>74</td>
<td><code>JobsCodec</code> request-response codec and length-prefixed blob framing</td>
</tr>
<tr>
<td><code>key_ops</code></td>
<td>206</td>
<td>Key artifact filesystem operations: validation, public-key-id derivation, distribution to parties, local key loading</td>
</tr>
</tbody></table>
<h3 id="mesh_gateway_wire"><code>mesh_gateway_wire</code></h3>
<p>The Face A / Face B vocabulary. Deliberately small and dependency-light so both sides of
every boundary can agree on it.</p>
<ul>
<li><strong>Face A</strong>: <code>FaceARequest</code> / <code>FaceAResponse</code> / <code>FaceABlobHeader</code>: job submission, blob
transfer, capability query. <code>CapabilityInfo</code> and <code>NodeType</code> are how a client tells a
gateway from an executor.</li>
<li><strong>Face B</strong>: <code>FaceBRequest</code> / <code>FaceBResponse</code> / <code>CryptoOp</code> / <code>CryptoJobState</code>:
crypto operations, with <code>PartyProtoResult</code> and <code>AggregatedJobResult</code> for threshold
aggregation.</li>
<li><strong>Jobs</strong>: <code>JobMetadata</code>, <code>JobType</code>, <code>JobState</code>, <code>Payload</code>, <code>JobResultPayload</code>;
<code>JobStore</code> / <code>JobRecord</code> persist job state across restarts.</li>
<li><strong>Artifacts</strong>: <code>ArtifactManifest</code>, <code>ArtifactEntry</code>, <code>SignedArtifact</code>, <code>SignedManifest</code>,
<code>ArtifactSignature</code>, <code>ArtifactAlias</code>, <code>VersionSelector</code>, and <code>verify_signed_artifact</code>.</li>
<li><code>blake3_blob_id</code> derives a <code>BlobId</code> from content.</li>
</ul>
<p>Serialization is <code>serde</code> + <code>bc2wrap</code> (Zama&#39;s bincode wrapper); libp2p carries opaque
<code>Vec&lt;u8&gt;</code> which each side decodes into these enums.</p>
<h3 id="meshes_config"><code>meshes_config</code></h3>
<blockquote>
<p><em>config for FHE p2p mesh</em></p>
</blockquote>
<p>Cluster configuration. Holds the Face A / Face B / jobs / blobs protocol constants, plus
<code>GatewayConfig</code> and the Mesh A config parsers. Ships the TOML cluster definitions:</p>
<table>
<thead>
<tr>
<th>File pattern</th>
<th>Describes</th>
</tr>
</thead>
<tbody><tr>
<td><code>mesh_a_cluster*.toml</code></td>
<td>Syndesmos + Autonomos nodes, Mesh A</td>
</tr>
<tr>
<td><code>mesh_c_cluster*.toml</code></td>
<td>Ypolo executors, Mesh C</td>
</tr>
<tr>
<td><code>mpc_cluster*.toml</code></td>
<td>Kryphos MPC parties, Mesh B</td>
</tr>
<tr>
<td><code>gateway_cluster*.toml</code></td>
<td>Pylon gateways, Face A + Face B</td>
</tr>
<tr>
<td><code>kentr_cluster*.toml</code></td>
<td>Kentr control nodes</td>
</tr>
<tr>
<td><code>default_N.toml</code></td>
<td>Per-party KMS core service configuration which is from KMS</td>
</tr>
</tbody></table>
<p>Each comes in bare (production), <code>_local</code>, and <code>_test</code> variants; <code>mpc_cluster_ext.toml</code> and
<code>mpc_cluster_gcp.toml</code> target external and Google Cloud deployments.</p>
<hr>
<h2 id="compute-framework">Compute framework</h2>
<p>A four-crate layering that keeps wire types, execution, program definition, and client
concerns separate.</p>
<h3 id="compute-abi"><code>compute-abi</code></h3>
<p>Wire types only, with no logic and no networking. <code>ComputeJobSpec</code>, <code>ComputeReceipt</code>,
<code>ProgramManifest</code>, <code>ProgramSlot</code>, <code>ProgramArtifactRequirement</code>, <code>ArtifactBinding</code>,
<code>InputPayload</code> (<code>Inline</code> or <code>BlobRef</code>), <code>InstalledProgramRecord</code>,
<code>ProgramPackageManifest</code>. Identity is <code>ProgramId</code> + <code>ProgramVersion</code>.</p>
<p><code>ProgramRuntimeKind</code> names four execution strategies: <code>NativeBuiltin</code>, <code>WasmModule</code>,
<code>ComputeGraphV1</code>, <code>ExternalProcessV1</code>. The <code>ComputeGraphV1*</code> types define a declarative
operation graph (ops, input/output bindings, typed payload fields); the
<code>ExternalProcess*V1</code> types define the protocol for a program running as a separate process.</p>
<h3 id="compute-core"><code>compute-core</code></h3>
<p>The executor contract, and almost nothing else:</p>
<pre><code class="language-rust">#[async_trait]
pub trait ComputeExecutor: Send + Sync {
    async fn execute(&amp;self, ctx: ComputeContext, exe_inputs: ExecutorInputs)
        -&gt; Result&lt;ComputeOutput&gt;;
}
</code></pre>
<p>With <code>ExecutorInputs</code> (spec, inputs, session inputs, artifacts), <code>ComputeSessionInput</code>
(kind, bytes, observed timestamp, sequence) and <code>ComputeOutput</code>.</p>
<h3 id="compute-program"><code>compute-program</code></h3>
<p>The program contract. A domain crate implements <code>SmartProgram</code>, owning its manifest, typed
ABI, and execution kernel:</p>
<pre><code class="language-rust">pub trait SmartProgram: Send + Sync + &#39;static {
    fn descriptor(&amp;self) -&gt; ProgramDescriptor;
    fn manifest(&amp;self) -&gt; ProgramManifest;
    fn execute(&amp;self, exec_inputs: ExecutorInputs) -&gt; Result&lt;ComputeOutput&gt;;

    // Session programs override this; Ypolo calls it as live inputs arrive and
    // runs the program once it returns Some.
    fn try_build_session_inputs(&amp;self, _: &amp;HashMap&lt;String, Vec&lt;u8&gt;&gt;)
        -&gt; Result&lt;Option&lt;Vec&lt;Vec&lt;u8&gt;&gt;&gt;&gt; { Ok(None) }
}
</code></pre>
<h3 id="compute-client"><code>compute-client</code></h3>
<p>Client-side SDK for submitting work:</p>
<table>
<thead>
<tr>
<th>Module</th>
<th>Provides</th>
</tr>
</thead>
<tbody><tr>
<td><code>discovery</code></td>
<td><code>discover_available_ypolo</code>, which registers at rendezvous, probes peers with <code>GetCapabilities</code>, returns an executor</td>
</tr>
<tr>
<td><code>upload</code></td>
<td><code>put_blob</code>, <code>register_artifact</code></td>
</tr>
<tr>
<td><code>submit</code></td>
<td><code>build_compute_job_spec</code>, <code>submit_compute_job</code>, <code>upload_input_blobs</code>, <code>upload_and_register_artifacts</code>, <code>push_program_session_input</code></td>
</tr>
<tr>
<td><code>poll</code></td>
<td><code>wait_for_job_result</code>, <code>fetch_receipt_bytes</code></td>
</tr>
<tr>
<td><code>deploy</code></td>
<td>Program installation onto an executor</td>
</tr>
</tbody></table>
<hr>
<h2 id="nodes">Nodes</h2>
<h3 id="node_syndesmos-mesh-a-rendezvous"><code>node_syndesmos</code>: Mesh A rendezvous</h3>
<p><strong>Binary:</strong> <code>node_syndesmos</code> · <strong>Config:</strong> <code>mesh_a_cluster*.toml</code> → <code>[[syndesmos]]</code></p>
<p>Bootstrap and rendezvous point. Fixed address and well-known peer ID; other nodes register
here to become discoverable. Binary-only, with no library. Also exposes an interactive
<code>store</code> / <code>call</code> / <code>get</code> prompt, so a rendezvous node doubles as a storage peer.</p>
<p>Deployed to Raspberry Pi Zero 2 W in the reference setup on OSATCON demo.</p>
<h3 id="node_autonomos-mesh-a-sensor-node"><code>node_autonomos</code>: Mesh A sensor node</h3>
<p><strong>Binary:</strong> <code>node_autonomos</code> · <strong>Config:</strong> <code>mesh_a_cluster*.toml</code> → <code>[[autonomos]]</code></p>
<p>The sensor node: gathers and encrypts data, and can store encrypted content to serve to
peers via the Merkle roots it publishes to the DHT. Its <code>encryption</code> module wraps the
store/retrieve paths; the crate re-exports <code>kryptografia</code> so the node can also act as an FHE
client, which is what the OSATCON edge operators build on. It pulls the <code>PublicKey</code>
artifact only, enough to encrypt but not to evaluate.</p>
<h3 id="node_kentr-mesh-a-control-plane"><code>node_kentr</code>: Mesh A control plane</h3>
<p><strong>Binary:</strong> <code>kentr</code> · <strong>Config:</strong> <code>kentr_cluster*.toml</code></p>
<p>Administrative and key-orchestration node, carrying <code>Admin</code> or <code>NonAdmin</code> role. It builds
its own <code>KomvosManager</code> from <code>libp2p_common</code> and leans on <code>plegma-fhe-pelatis</code> for the FHE
key and decryption lifecycle. It pulls <strong>all three</strong> key kinds, where an Autonomos takes
only the <code>PublicKey</code>.</p>
<table>
<thead>
<tr>
<th>Module</th>
<th>Role</th>
</tr>
</thead>
<tbody><tr>
<td><code>mesh_cmd</code></td>
<td>Mesh commands, including <code>fetch_keyset_artifacts_from_pylon</code></td>
</tr>
<tr>
<td><code>key_artifacts</code></td>
<td>Keyset artifact handling</td>
</tr>
<tr>
<td><code>key_rehydration</code></td>
<td>Restoring key material into a node (<code>--rehydrate-key</code>)</td>
</tr>
</tbody></table>
<p>Wraps Zama&#39;s <code>kms_core_client</code>, so <code>kentr</code> also drives KMS operations:
<code>preproc-key-gen</code>, <code>key-gen</code>, <code>key-gen-fresh</code>, <code>user-decrypt</code>, <code>crs-gen</code>.</p>
<h3 id="node_kryphos-mesh-b-mpc-party"><code>node_kryphos</code>: Mesh B MPC party</h3>
<p><strong>Binary:</strong> <code>node_fhe</code> · <strong>Config:</strong> <code>mpc_cluster*.toml</code> → <code>[[node]]</code></p>
<p><strong>The most significant modification to upstream Zama code.</strong> A KMS threshold party whose
inter-party transport is libp2p instead of gRPC. Builds <code>RealCentralizedKms</code> /
<code>new_real_threshold_kms</code> with <code>P2pInputs</code>, handles attestation via
<code>SecurityModuleProxy</code>, and manages vaults and keychains (local, AWS KMS, S3).</p>
<p>Each party runs <strong>two managers on two separate listeners</strong>. See
<a href="/docs/architecture/#mesh-b--threshold-fhe-and-key-management">ARCHITECTURE.md §2</a>:</p>
<table>
<thead>
<tr>
<th>Module</th>
<th>Manager</th>
<th>Face</th>
<th>Listener</th>
<th>Protocol</th>
</tr>
</thead>
<tbody><tr>
<td><code>node_kryphos</code></td>
<td><code>KryphosManager</code></td>
<td>face A, internal, MPC rounds between parties</td>
<td><code>listen_address</code></td>
<td><code>/threshold-fhe/1</code></td>
</tr>
<tr>
<td><code>pylon</code></td>
<td><code>KryphosPylonManager</code></td>
<td>face B, external, serves the gateway only</td>
<td><code>gateway_listen_addr</code></td>
<td><code>/mesh/gateway/crypto/1</code></td>
</tr>
<tr>
<td><code>kryphos_config</code></td>
<td>n/a</td>
<td>party configuration</td>
<td></td>
<td></td>
</tr>
</tbody></table>
<p>Pylon&#39;s Face B connects to each party&#39;s face B; the MPC rounds happen entirely on the face
A listeners, where the gateway has no presence.</p>
<p>Requires the <code>p2p</code> feature. Documented runs also pass <code>insecure</code>. See
<a href="/security/#insecure-is-a-default-feature">SECURITY.md</a>.</p>
<h3 id="node_pylon-the-gateway"><code>node_pylon</code>: the gateway</h3>
<p><strong>Binary:</strong> <code>node_pylon</code> · <strong>Config:</strong> <code>gateway_cluster*.toml</code> + <code>mpc_cluster*.toml</code></p>
<p><code>GatewayManager</code> runs both faces and the job store.</p>
<table>
<thead>
<tr>
<th>Module</th>
<th>Role</th>
</tr>
</thead>
<tbody><tr>
<td><code>face_a_jobs</code></td>
<td>Mesh A job control (server side)</td>
</tr>
<tr>
<td><code>face_a_blobs</code>, <code>face_a_blobs_client</code></td>
<td>Blob transfer, both directions</td>
</tr>
<tr>
<td><code>face_b</code></td>
<td><code>FaceBManager</code>, client to the Kryphos parties (Kryphos&#39; face B only)</td>
</tr>
<tr>
<td><code>runtime</code></td>
<td><code>GatewayRuntime</code>, job execution and result aggregation</td>
</tr>
<tr>
<td><code>kryphos_config_copy</code></td>
<td>Local copy of party config parsing</td>
</tr>
</tbody></table>
<p>Tracks discovered Mesh A peers (excluding rendezvous nodes) and applies the
<code>num_majority</code> / <code>num_reconstruct</code> threshold rules when aggregating party responses.</p>
<h3 id="node_ypolo-mesh-c-executor"><code>node_ypolo</code>: Mesh C executor</h3>
<p><strong>Binary:</strong> <code>ypolo</code> · <strong>Config:</strong> <code>mesh_c_cluster*.toml</code> → <code>[[ypolo]]</code></p>
<p>Serves Face A for compute jobs, resolves inputs and artifacts, runs programs, and issues
signed receipts. Holds the <code>ServerKey</code> (evaluation key), never a decryption key. It builds
its own <code>komvos</code> client and stands alongside the other node crates rather than on top of
any of them.</p>
<p>No other crate in the workspace depends on <code>node_ypolo</code>: callers reach it over the wire,
never by linking it. The compute boundary is enforced in the build graph as well as at
runtime.</p>
<table>
<thead>
<tr>
<th>Module</th>
<th>Role</th>
</tr>
</thead>
<tbody><tr>
<td><code>node_ypolo</code></td>
<td><code>YpoloManager</code>: lifecycle, jobs handler, event loop, <code>configure_tfhe_executor</code></td>
</tr>
<tr>
<td><code>executor_tfhe</code></td>
<td>In-process TFHE evaluation</td>
</tr>
<tr>
<td><code>executor_external_process</code></td>
<td>Runs a program as a child process (<code>ExternalProcessV1</code>)</td>
</tr>
<tr>
<td><code>programs</code>, <code>program_manifest</code></td>
<td>Program registry and manifests</td>
</tr>
<tr>
<td><code>artifacts</code></td>
<td>Artifact resolution and caching</td>
</tr>
<tr>
<td><code>receipt</code></td>
<td><code>ComputeReceipt</code> construction</td>
</tr>
</tbody></table>
<hr>
<h2 id="client">Client</h2>
<h3 id="plegma-fhe-pelatis"><code>plegma-fhe-pelatis</code></h3>
<blockquote>
<p><em>libp2p transport for ZAMA KMS</em>, the &quot;mesh FHE client&quot;</p>
</blockquote>
<p>The full client-side FHE lifecycle, and the crate an integrator is most likely to use
directly. Despite sitting under &quot;Client&quot; here, it is a <strong>library beneath the nodes</strong>:
<code>node_kentr</code> and <code>headquarter</code> both depend on it. It does not depend on <code>kryptografia</code>;
the two are parallel entry points to FHE, <code>kryptografia</code> for encrypting with a public key
and <code>plegma-fhe-pelatis</code> for the keyset and decryption lifecycle against the gateway.</p>
<table>
<thead>
<tr>
<th>Module</th>
<th>Role</th>
</tr>
</thead>
<tbody><tr>
<td><code>encrypt</code></td>
<td><code>encrypt_with_gateway</code>, cipher parameters, CRS materialization</td>
</tr>
<tr>
<td><code>decrypt</code></td>
<td>Public and user decryption: request construction, submission, result polling, <code>PublicDecryptAggregate</code> / <code>UserDecryptAggregate</code></td>
</tr>
<tr>
<td><code>key_material</code></td>
<td>Key material acquisition and lifecycle</td>
</tr>
<tr>
<td><code>artifact_cache</code></td>
<td><code>ArtifactCache</code>, local caching of keys and CRS</td>
</tr>
<tr>
<td><code>core_client_bridge</code></td>
<td><code>MeshCoreClientBridge</code>, which builds KMS requests (<code>build_keygen_request</code>, <code>build_crsgen_request</code>, <code>build_preproc_request</code>, <code>build_public_decrypt_request</code>, <code>build_user_decrypt_request</code>)</td>
</tr>
<tr>
<td><code>pylon_client</code></td>
<td>Face A client to the gateway</td>
</tr>
<tr>
<td><code>threshold_client</code></td>
<td>Threshold-party interaction</td>
</tr>
<tr>
<td><code>materialize</code></td>
<td>Turning fetched artifacts into usable key objects</td>
</tr>
<tr>
<td><code>verify</code></td>
<td>Artifact and manifest signature verification</td>
</tr>
<tr>
<td><code>discovery</code></td>
<td>Gateway discovery</td>
</tr>
</tbody></table>
<hr>
<h2 id="reference-application-osatcon">Reference application: <code>osatcon/</code></h2>
<p>An encrypted convoy/satellite exposure alerting system in which a convoy asks &quot;am I about to be
observed?&quot; without anyone pooling the route with the satellite ephemeris. See
<a href="/docs/architecture/#6-reference-application-osatcon">ARCHITECTURE.md §6</a>.</p>
<h3 id="ofield-core"><code>ofield-core</code></h3>
<p>Domain mathematics, no networking. <code>geo</code> / <code>geo_conv</code> (geodesy, coordinate conversion),
<code>projection</code> (local equirectangular), <code>convoy</code> (route interpolation), <code>satellites</code>
(tracks), <code>paths</code> (corridors), <code>alert_math</code> / <code>alert_math_conv</code> (exposure geometry, in
plaintext and FHE-convertible forms), and <code>sync_scenario</code>, which defines <code>OsatconScenarioV1</code>, a
versioned scenario descriptor (tick rate, alert radius, projection reference latitude,
coordinate scale, convoy speed) that every participant hashes to confirm they are computing
against identical parameters.</p>
<h3 id="smart-program-alert"><code>smart-program-alert</code></h3>
<p><strong>Binaries:</strong> <code>smart-program-alert</code> (the program), <code>deploy_alert</code> (the installer)</p>
<p>The FHE smart program <strong><code>satcon.alert_math</code> v0.1.0</strong>. Implements <code>SmartProgram</code>, computing
exposure over <code>FheBool</code> and <code>FheUint64</code> with TFHE safe serialization. Session-oriented:
consumes three input kinds (<code>config_v1</code>, <code>convoy_v1</code>, <code>satellite_v1</code>) as they stream in.
<code>orchestrator</code> sequences the computation, <code>graph</code> defines input kinds and the operation
graph, <code>deploy</code> installs the program onto an Ypolo executor.</p>
<h3 id="edge-convoy-operator"><code>edge-convoy-operator</code></h3>
<p><strong>Binary:</strong> <code>edge-convoy-operator</code></p>
<p>Field terminal for the convoy. Contributes encrypted positions; <code>convoy_ui</code> provides the
operator display. Cross-compiled to Raspberry Pi 5 (aarch64) and Pi Zero 2 W (armv7).</p>
<h3 id="edge-satellite-operator"><code>edge-satellite-operator</code></h3>
<p><strong>Binary:</strong> <code>edge-satellite-operator</code></p>
<p>Field terminal for satellite tracking. Contributes encrypted tracks. Cross-compiled to
x86_64 Linux; the reference build targets a 2011 MacBook running Kali, deliberately.</p>
<h3 id="headquarter"><code>headquarter</code></h3>
<p><strong>Binary:</strong> <code>headquarter</code></p>
<p>Command view. Requests decryption of alerts through Kentr and renders the operational
picture. Supports <code>--ui-only</code> for display without a live mesh.</p>
<hr>
<h2 id="root-crate-and-vendored-code">Root crate and vendored code</h2>
<h3 id="mesh-workspace-root"><code>mesh</code> (workspace root)</h3>
<p><strong>Binaries:</strong> <code>mesh</code> (default), <code>meshfhe</code></p>
<p>Top-level entry point. Features: <code>default = [&quot;p2p&quot;, &quot;insecure&quot;]</code>, where <code>p2p</code> enables
<code>kms/p2p</code> and <code>insecure</code> enables <code>kms/insecure</code>. <code>src/old_but_keep/</code> holds superseded
implementations retained for reference; nothing live calls into it.</p>
<h3 id="kms"><code>kms/</code></h3>
<p>Vendored copy of <a href="https://github.com/zama-ai/kms/tree/v0.12.3">Zama KMS v0.12.3</a>, modified
for libp2p transport. Not a workspace member in its own right; consumed through
<code>kms/core/service</code>, <code>kms/core/grpc</code>, <code>kms/core/threshold</code>, <code>kms/observability</code>,
<code>kms/bc2wrap</code>.</p>
<p>Its own binaries (<code>kms-init</code>, <code>kms-server</code>, <code>kms-gen-keys</code>, <code>kms-gen-tls-certs</code>,
<code>kms-custodian</code>, <code>kms-core-client</code>) are used directly during setup.</p>
<blockquote class="docs-alert docs-alert-warning"><p class="docs-alert-title">Warning</p><p><br>
<strong>The KMS was designed around gRPC, and that assumption runs deep.</strong><br>
It is not confined to
a transport module. It shapes the architecture and the naming throughout the code, so
concepts and methods are entangled with it in ways that are not obvious from a single
file. Making libp2p take over therefore meant modifying many files and adding others.</p>
<p><strong>Nothing from the original has been deleted.</strong> Upstream code sits alongside the ØMYSTIK
modifications, so a file existing here does not mean it is on a live path. Before
modifying KMS, check before
assuming either that a labeled component as &quot;gRPC&quot; is actually a gRPC component, a larger
tool, is a dead component or that it has been replaced.</p>
</blockquote>
<p><strong>Licensing and maintenance obligations attach to this directory.</strong> See
<a href="/licensing/#2-zama-components-commercial-use-requires-an-agreement">LICENSING.md §2</a>.</p>
`},{route:`/docs/cryptography/`,source:`docs/CRYPTOGRAPHY.md`,title:`Cryptographic Inventory`,html:`<h1 id="cryptographic-inventory">Cryptographic Inventory</h1>
<p>Every cryptographic primitive ØMYSTIK uses, what it protects, and where the keys live.</p>
<p>Written for auditors, integrators, and anyone completing a regulatory declaration about the
software. For how the pieces fit together, read <a href="/docs/architecture/">ARCHITECTURE.md</a>; for the
security posture and its known weaknesses, <a href="/security/">SECURITY.md</a>.</p>
<blockquote class="docs-alert docs-alert-note"><p class="docs-alert-title">Note</p><p>ØMYSTIK implements no cryptographic primitive of its own. Every algorithm below comes
from an established library: Zama&#39;s TFHE-rs and KMS, the RustCrypto suite, BLAKE3,
rust-libp2p, or rustls with its <code>aws-lc-rs</code> provider. What ØMYSTIK contributes is the
composition: which primitive protects what, and how keys move between nodes.</p>
<p>&quot;Established&quot; carries one qualification. The rustls on the transport path is a third-party
fork, not the crates.io release. See <a href="#6-provenance">§6</a>.</p>
</blockquote>
<hr>
<h2 id="1-what-the-software-does-cryptographically">1. What the software does, cryptographically</h2>
<p>Three separate protections, easily confused because they all involve encryption:</p>
<table>
<thead>
<tr>
<th></th>
<th>Protects</th>
<th>Primitive</th>
<th>Key held by</th>
</tr>
</thead>
<tbody><tr>
<td><strong>Stored content</strong></td>
<td>Files at rest on a node</td>
<td>XChaCha20-Poly1305</td>
<td>The user who stored the file, never the mesh</td>
</tr>
<tr>
<td><strong>Transport</strong></td>
<td>Traffic between nodes</td>
<td>TLS 1.3, inside QUIC</td>
<td>Each node, per session</td>
</tr>
<tr>
<td><strong>Computation</strong></td>
<td>Data <em>while being computed on</em></td>
<td>TFHE (fully homomorphic)</td>
<td>Split across MPC parties; never assembled</td>
</tr>
</tbody></table>
<p>The third is the unusual one. Data submitted for computation is encrypted under a threshold
FHE public key, evaluated in ciphertext form by a node that holds only an evaluation key,
and decrypted only when a quorum of independent parties cooperates.</p>
<hr>
<h2 id="2-algorithms">2. Algorithms</h2>
<h3 id="21-own-crates">2.1 Own crates</h3>
<table>
<thead>
<tr>
<th>Algorithm</th>
<th>Mode</th>
<th>Key / output size</th>
<th>Function</th>
</tr>
</thead>
<tbody><tr>
<td><strong>XChaCha20-Poly1305</strong></td>
<td>AEAD, streamed in 2 MiB chunks (<code>EncryptorBE32</code> / <code>DecryptorBE32</code>)</td>
<td>key 256 bits, nonce 152 bits</td>
<td>Confidentiality + integrity of stored files</td>
</tr>
<tr>
<td><strong>BLAKE3</strong></td>
<td>Hash</td>
<td>256-bit output</td>
<td>Content identifier (CID) derivation; transfer integrity; sparse Merkle tree hashing</td>
</tr>
<tr>
<td><strong>SHA-256</strong></td>
<td>Hash</td>
<td>256-bit output</td>
<td>Metadata Merkle tree (<code>rs_merkle</code>), producing the MID</td>
</tr>
<tr>
<td><strong>Ed25519</strong></td>
<td>Signature</td>
<td>256 bits</td>
<td>libp2p peer identity, carried in the QUIC TLS certificate</td>
</tr>
<tr>
<td><strong>ECDSA / secp256k1</strong></td>
<td>Signature</td>
<td>256 bits</td>
<td>KMS signing keys, artifact signatures</td>
</tr>
<tr>
<td><strong>TLS 1.3, inside QUIC</strong></td>
<td>X25519 key agreement (P-256, P-384 as fallbacks); one of <code>TLS13_CHACHA20_POLY1305_SHA256</code>, <code>TLS13_AES_256_GCM_SHA384</code>, <code>TLS13_AES_128_GCM_SHA256</code></td>
<td>256 bits</td>
<td>Transport encryption and peer authentication, on every link including between MPC parties</td>
</tr>
</tbody></table>
<blockquote class="docs-alert docs-alert-note"><p class="docs-alert-title">Note</p><p><strong>Noise is not used, despite being a libp2p default that readers expect.</strong> Every swarm in
this workspace is built <code>.with_tokio().with_quic()</code> and nothing else: see
<code>libp2p_common/src/komvos.rs</code>, <code>ypolo.rs</code> and <code>kryphos_dual.rs</code>. There is no TCP transport
and therefore no Noise handshake, and every configured multiaddr is <code>/udp/&lt;port&gt;/quic-v1</code>.
The <code>noise</code> feature in <code>Cargo.toml</code> is enabled but never reaches a code path. Transport
security is TLS 1.3 as QUIC provides it, with the peer&#39;s Ed25519 identity bound in through
the libp2p certificate extension.</p>
</blockquote>
<p>The transport row is exact rather than indicative. <code>libp2p-tls</code> 0.5.0 restricts the
connection in three ways: TLS 1.3 only, the three cipher suites named above and no others,
and the <strong>ring</strong> crypto provider. It does not override the key exchange groups, so ring&#39;s
defaults apply: X25519 first, then P-256 and P-384. <strong>No hybrid post-quantum group is
offered.</strong> Newer rustls builds on <code>aws-lc-rs</code> can negotiate <code>X25519MLKEM768</code>, but the ring
provider has no ML-KEM at all, so it cannot arise here. Between two ØMYSTIK nodes the
agreement is X25519.</p>
<h3 id="22-vendored-zama-kms">2.2 Vendored Zama KMS</h3>
<p>Reachable in the shipped artifact, not merely declared in the manifest. See
<a href="https://github.com/omystik/omystik/blob/main/kms/UPSTREAM.md">kms/UPSTREAM.md</a> for provenance.</p>
<table>
<thead>
<tr>
<th>Algorithm</th>
<th>Mode</th>
<th>Key / output size</th>
<th>Function</th>
</tr>
</thead>
<tbody><tr>
<td><strong>TFHE</strong></td>
<td>Fully homomorphic encryption</td>
<td>per DKG parameter set</td>
<td>Computation over ciphertext</td>
</tr>
<tr>
<td><strong>Threshold secret sharing</strong></td>
<td>MPC, <em>n</em> = 3<em>t</em> + 1</td>
<td>n/a</td>
<td>Distributed key generation and decryption</td>
</tr>
<tr>
<td><strong>ML-KEM (FIPS 203)</strong></td>
<td>Post-quantum KEM</td>
<td>ML-KEM-512 in use; ML-KEM-1024 for legacy key deserialisation only</td>
<td>Hybrid encryption for signcryption and backup</td>
</tr>
<tr>
<td><strong>AES-256-GCM</strong></td>
<td>AEAD</td>
<td>256 bits</td>
<td>Symmetric encryption within KMS</td>
</tr>
<tr>
<td><strong>AES-256-GCM-SIV</strong></td>
<td>AEAD, nonce-misuse resistant</td>
<td>256 bits</td>
<td>Keychain / vault protection</td>
</tr>
<tr>
<td><strong>AES-based PRNG</strong></td>
<td>CSPRNG</td>
<td>256 bits</td>
<td>Deterministic randomness for MPC protocols</td>
</tr>
<tr>
<td><strong>ECDSA / secp256k1</strong></td>
<td>Signature</td>
<td>256 bits</td>
<td>Core signing key, EIP-712</td>
</tr>
<tr>
<td><strong>ECDSA / P-384</strong></td>
<td>Signature</td>
<td>384 bits</td>
<td>Certificate paths</td>
</tr>
<tr>
<td><strong>RSA</strong></td>
<td>Encryption (recipient keypair)</td>
<td>2048 bits</td>
<td>AWS KMS keychain interoperability</td>
</tr>
<tr>
<td><strong>SHA-3</strong></td>
<td>Hash</td>
<td>n/a</td>
<td>Within KMS protocols</td>
</tr>
<tr>
<td><strong>Zero-knowledge proofs</strong></td>
<td><code>tfhe-zk-pok</code></td>
<td>n/a</td>
<td>Proofs of correct ciphertext formation</td>
</tr>
</tbody></table>
<p>Presence does not mean every deployment exercises every algorithm: the AWS-facing paths
(RSA, parts of the keychain) are inactive unless AWS KMS or S3 storage is configured.</p>
<hr>
<h2 id="3-key-lifecycle">3. Key lifecycle</h2>
<h3 id="31-per-file-content-keys">3.1 Per-file content keys</h3>
<p>Generated by <code>content_hashing::newkey()</code> from the operating system CSPRNG (<code>OsRng</code>): a
256-bit key and a 152-bit nonce, fresh for every file.</p>
<p><code>store_file</code> returns them to the caller and <strong>the mesh retains no copy.</strong> There is no
recovery path. Losing them makes the content permanently unreadable, and leaking them
exposes that file and no other.</p>
<p>The wider model these keys sit in, MID, CID and the two Merkle layers, is described in
<a href="/readme-mystik-p2p/">README_mystik-p2p.md</a>.</p>
<p>Because the key and nonce are fresh per file, encrypting identical plaintext twice produces
different ciphertext and therefore a different CID. The CID is BLAKE3 over the <em>ciphertext</em>,
so it reveals nothing about the plaintext and cannot be used to correlate identical files
held by different nodes.</p>
<h3 id="32-threshold-fhe-keys">3.2 Threshold FHE keys</h3>
<p>Generated by the MPC parties as a distributed protocol. The private key exists only as
shares; reconstruction requires a quorum, and no party ever holds the whole key, not even
the gateway that aggregates their responses.</p>
<p>Three artifacts are distributed outward from that process:</p>
<table>
<thead>
<tr>
<th>Artifact</th>
<th>Distributed to</th>
<th>Enables</th>
</tr>
</thead>
<tbody><tr>
<td><code>PublicKey</code></td>
<td>any node that encrypts</td>
<td>Encrypting inputs</td>
</tr>
<tr>
<td><code>ServerKey</code></td>
<td>compute executors (Ypolo)</td>
<td>Evaluating over ciphertext, <strong>not</strong> decryption</td>
</tr>
<tr>
<td><code>PublicKeyMetadata</code></td>
<td>control nodes</td>
<td>Parameter agreement</td>
</tr>
</tbody></table>
<p>Acquisition is described in <a href="/docs/architecture/#how-nodes-find-each-other">ARCHITECTURE.md §2</a>:
originals come from the Pylon gateway over Face A <code>GetArtifact</code>; replicas propagate between
Mesh A peers over <code>/key/1</code>.</p>
<h3 id="33-node-identities">3.3 Node identities</h3>
<p>Each node has a libp2p Ed25519 keypair. In the committed example configurations these are
derived from a one-byte <code>secret_seed</code>, which is <strong>reproducible by anyone</strong> and suitable only
for local testing. See
<a href="/security/#deterministic-node-identities-from-short-seeds">SECURITY.md</a>.</p>
<h3 id="34-transport-and-mpc-certificates">3.4 Transport and MPC certificates</h3>
<p><strong>MPC parties authenticate each other the same way every other pair of nodes does</strong>, through
QUIC&#39;s TLS 1.3 and the Ed25519 peer identity carried in the libp2p certificate. There is no
separate authentication mechanism for Mesh B.</p>
<p>The upstream KMS protects the party-to-party link with mTLS over gRPC instead, and that
machinery is still present: <code>kms-gen-tls-certs</code> generates one CA per party, the shipped
<code>kms/core/service/config/default_*.toml</code> carry a <code>[threshold.tls.manual]</code> block with
<code>cert_p1.pem</code> … <code>cert_p4.pem</code>, and <code>build_tls_config</code> in <code>node_kryphos/src/lib.rs</code> reads them
at startup. <strong>None of it is in effect.</strong> <code>kms_impl.rs</code> selects the transport at compile time,
and the TLS configuration is consumed only under <code>#[cfg(not(feature = &quot;p2p&quot;))]</code>, while the
default build is <code>default = [&quot;p2p&quot;, &quot;insecure&quot;]</code>. On the p2p branch the certificates are
built, passed to <code>new_real_threshold_kms</code>, and never used.</p>
<p>Two consequences worth knowing if you operate this:</p>
<ul>
<li>The TCP socket bound by <code>make_mpc_listener</code> is never served under p2p. It must still bind,
or the process panics, for a listener nothing reads.</li>
<li>The log line <em>&quot;No TLS identity, using plaintext communication between MPC nodes&quot;</em> is
misleading here. Omitting TLS configuration does not produce plaintext, it produces
QUIC-encrypted traffic.</li>
</ul>
<hr>
<h2 id="4-data-handling-around-encryption">4. Data handling around encryption</h2>
<h3 id="41-before-encryption">4.1 Before encryption</h3>
<p>No compression, no format conversion, no header insertion. Plaintext is read from disk and
streamed into the AEAD in 2 MiB chunks; it is never held in memory in full.</p>
<h3 id="42-after-encryption">4.2 After encryption</h3>
<p>The ciphertext is written as <code>&lt;CID&gt;.bin</code>, where the CID is the BLAKE3 hash of the ciphertext
stream. No header, no envelope, no metadata is attached to the encrypted file; the
metadata lives separately, in the Merkle structures described in
<a href="/docs/architecture/#4-how-file-storage-and-exchange-works">ARCHITECTURE.md §4</a>.</p>
<p>For transport, ciphertext is framed by the libp2p protocol in use (<code>/file-exchange/1</code> for
files, <code>/mesh/gateway/blob/1</code> for compute blobs) and hashed with BLAKE3 at both ends.</p>
<p>FHE ciphertexts use TFHE safe serialization
(<code>tfhe.safe_serialize.CompactCiphertextList.v1</code>), which carries its own versioned header.</p>
<hr>
<h2 id="5-protecting-the-encryption-process">5. Protecting the encryption process</h2>
<p>What exists today, stated plainly:</p>
<ul>
<li><strong>Streaming AEAD.</strong> Each chunk is independently authenticated, so truncation or
reordering is detected rather than silently decrypted.</li>
<li><strong>Transfer verification.</strong> BLAKE3 on both sides of every file transfer.</li>
<li><strong>Signed artifacts.</strong> Key artifacts and program manifests carry signatures, verified
through <code>verify_signed_artifact</code>.</li>
<li><strong>Threshold custody.</strong> No single party can decrypt, by construction rather than by
policy.</li>
</ul>
<p>What does not exist: no hardware key storage (the vendored KMS carries an AWS Nitro enclave
path, which no ØMYSTIK configuration enables), no tamper detection on the software itself,
and no runtime integrity checking of the binary.</p>
<p>Six TLS-related dependencies come from third-party git branches rather than crates.io. To be
precise about what that costs, because it is easy to overstate: <code>Cargo.lock</code> does record an
exact commit for each, so a build from the committed lockfile is deterministic. The exposure
is that the <em>manifest</em> names a mutable branch, so <code>cargo update</code> silently follows wherever
that branch has moved; that reproducing any build depends on a third party keeping those
commits reachable; and that the code itself is unaudited and outside the crates.io supply
chain. The root and <code>kms/</code> lockfiles already disagree on three of them. See
<a href="#6-provenance">§6</a> and
<a href="/security/#cryptographic-dependencies-use-forks">SECURITY.md</a>.</p>
<hr>
<h2 id="6-provenance">6. Provenance</h2>
<table>
<thead>
<tr>
<th>Source</th>
<th>Supplies</th>
</tr>
</thead>
<tbody><tr>
<td><a href="https://github.com/zama-ai/tfhe-rs">Zama TFHE-rs</a> 1.4.0-alpha.3</td>
<td>FHE</td>
</tr>
<tr>
<td><a href="https://github.com/zama-ai/kms">Zama KMS</a> v0.12.3, vendored</td>
<td>Threshold key management, MPC</td>
</tr>
<tr>
<td><a href="https://github.com/libp2p/rust-libp2p">rust-libp2p</a> 0.54.1</td>
<td>QUIC transport, peer identity, <code>libp2p-tls</code> 0.5.0 and <code>libp2p-quic</code> 0.11.1</td>
</tr>
<tr>
<td><code>rustls</code> 0.23.31, <strong>from a git branch</strong>, with <code>quinn</code> 0.11.9</td>
<td>The TLS 1.3 protocol behind QUIC; primitives come from its provider, <code>ring</code></td>
</tr>
<tr>
<td><code>ring</code> 0.17.14</td>
<td>rustls&#39; crypto provider <strong>on the transport path</strong>: X25519, the TLS 1.3 AEADs, certificate signature verification</td>
</tr>
<tr>
<td><code>aws-lc-rs</code> 1.15.1</td>
<td>rustls&#39; crypto provider on the KMS mTLS path, installed as the process default in <code>node_kryphos</code></td>
</tr>
<tr>
<td><a href="https://github.com/RustCrypto">RustCrypto</a></td>
<td>ChaCha20-Poly1305, AES-GCM, AES-GCM-SIV, SHA-2, SHA-3, ML-KEM, k256, p384, RSA</td>
</tr>
<tr>
<td><a href="https://github.com/BLAKE3-team/BLAKE3">BLAKE3</a></td>
<td>Hashing</td>
</tr>
</tbody></table>
<blockquote class="docs-alert docs-alert-warning"><p class="docs-alert-title">Warning</p><p><strong>The <code>rustls</code> on the transport path is not the crates.io one.</strong> <code>[patch.crates-io]</code>
redirects it to <code>mkmks/rustls</code> branch <code>k256</code>, revision <code>2e446f8</code>, and <code>Cargo.lock</code> shows
<code>quinn</code>, <code>quinn-proto</code> and <code>libp2p-tls</code> all resolving to that patched <code>0.23.31</code>. QUIC being
the only transport, this means every handshake and every encrypted exchange between every
pair of nodes, including between MPC parties, is driven by a TLS implementation on a
mutable third-party branch that is outside crates.io and unaudited. The primitives
themselves come from <code>ring</code>, unpatched; what the fork controls is the protocol around
them, which is enough to matter.</p>
<p>The same applies to <code>rustls-pki-types</code>, <code>rustls-webpki</code>, <code>tokio-rustls</code>, <code>rcgen</code> and
<code>attestation-doc-validation</code>. See
<a href="/security/#cryptographic-dependencies-use-forks">SECURITY.md</a>.</p>
</blockquote>
<p><strong>What the fork actually changes.</strong> The branch name is <code>k256</code>, and it does what the name
says: it teaches the TLS certificate stack ECDSA over <strong>secp256k1</strong>, exposed as
<code>webpki::aws_lc_rs::ECDSA_P256K1_SHA256</code> and rcgen&#39;s <code>PKCS_ECDSA_P256K1_SHA256</code>. Neither
exists upstream, because secp256k1 is not an IANA-registered TLS signature scheme. Zama needs
it so a party&#39;s TLS certificate can be signed with the same secp256k1 key the KMS already
uses for EIP-712. The underlying primitives remain a provider&#39;s; the fork adds which
signature algorithm the certificate path will accept, not new arithmetic.</p>
<p><strong>Two providers are in the build, and they serve different paths.</strong> <code>node_kryphos</code> installs
<code>aws-lc-rs</code> as the process-wide default, which is what the mTLS path picks up, and the
secp256k1 additions are exposed as <code>webpki::aws_lc_rs::ECDSA_P256K1_SHA256</code>. But
<code>libp2p-tls</code> does not use the process default: it constructs its own provider from <strong>ring</strong>
explicitly. So the transport, which is the only path that runs, is served by ring, and
<code>aws-lc-rs</code> serves the path that does not.</p>
<p>There is an irony worth recording. Every call site for the secp256k1 additions sits on the
mTLS path, in <code>kms/core/threshold/src/tls_certs.rs</code>, <code>kms-server.rs</code> and <code>build_tls_config</code>,
and §3.4 explains that path is not the one ØMYSTIK runs. So the forked TLS stack is carried
across the entire transport to support a feature only the unused branch needs. Reverting to
crates.io rustls is therefore worth <em>investigating</em> rather than assuming impossible, though
the patch is inherited from the vendored KMS and both workspaces would have to agree.</p>
<p>Full licence terms and the Zama commercial-use obligation: <a href="/licensing/">LICENSING.md</a>.</p>
<hr>
<h2 id="7-status">7. Status</h2>
<p>ØMYSTIK has <strong>not</strong> undergone independent cryptographic review, and the default build
enables an <code>insecure</code> feature that relaxes protections in the KMS. Nothing in this document
should be read as an assurance that the composition is correct, only as an accurate
description of what it is. See <a href="/security/">SECURITY.md</a>.</p>
`},{route:`/docs/getting-started/`,source:`docs/GETTING_STARTED.md`,title:`Getting Started`,html:`<h1 id="getting-started">Getting Started</h1>
<p>How to build ØMYSTIK and bring up a working local cluster.</p>
<blockquote class="docs-alert docs-alert-note"><p class="docs-alert-title">Note</p><p>We use 4 nodes (4 MPC parties) based on the $3t + 1$ formula, where $t$ is the max number of
compromised nodes. </p>
</blockquote>
<blockquote class="docs-alert docs-alert-warning"><p class="docs-alert-title">Warning</p><p>Every command here builds with the <code>insecure</code> feature, which is in the workspace default
feature set. This is a <strong>development</strong> configuration and produces binaries unsuitable for
production. See <a href="/security/#insecure-is-a-default-feature">SECURITY.md</a>.</p>
</blockquote>
<hr>
<h2 id="1-prerequisites">1. Prerequisites</h2>
<ul>
<li><strong>Rust</strong>: recent stable toolchain, edition 2021</li>
<li><strong>Build essentials</strong>: a C toolchain, for the native code in the cryptographic
dependencies</li>
<li><strong>Disk and time</strong>: the workspace is large and the FHE dependencies are heavy. Expect a
long first build and several GB in <code>target/</code>.</li>
<li><strong><a href="https://github.com/cross-rs/cross"><code>cross</code></a></strong>: only if cross-compiling for edge
hardware (<a href="#8">§8</a>)</li>
</ul>
<pre><code class="language-bash">cargo build --workspace
</code></pre>
<p><code>kms/</code> is a <strong>separate Cargo workspace</strong> with its own members, and none of its crates are
members of the root one. <code>cargo locate-project --workspace</code> resolves to <code>kms/Cargo.toml</code>
from <code>kms/core/service</code>, and to the root manifest from <code>node_kryphos</code>.<br>
<code>--workspace</code> therefore builds ØMYSTIK&#39;s 24 crates, pulling in <code>kms/core/api</code>,
<code>kms/core/service</code>, <code>kms/core/grpc</code>, <code>kms/core/threshold</code>, <code>kms/core/libp2p</code>,
<code>kms/observability</code> and <code>kms/bc2wrap</code> as ordinary path dependencies. The two workspaces do
not conflict.</p>
<p>To build just one part, name it rather than the workspace:</p>
<pre><code class="language-bash">cargo build -p node_kryphos --bin node_fhe --features &quot;p2p insecure&quot;
</code></pre>
<p>Because <code>[patch.crates-io]</code> points six TLS and attestation crates at git branches, the
first build needs network access to those repositories. See
<a href="/security/#cryptographic-dependencies-use-forks">SECURITY.md</a> for why that matters.</p>
<h2 id="2-kms-configuration-server-kryphos-face-a-and-client">2. KMS configuration: Server (Kryphos&#39; face A) and Client</h2>
<ul>
<li>mpc_party face A (<a href="https://github.com/omystik/omystik/blob/main/kms/core/service/config/default_1.toml">kms/core/service/config</a>)</li>
</ul>
<pre><code>.
├── default_1.toml
├── default_2.toml
├── default_3.toml
├── default_4.toml
</code></pre>
<ul>
<li>actual kentr admin (<a href="https://github.com/omystik/omystik/blob/main/node_kentr/config">node_kentr/config</a>)</li>
</ul>
<pre><code>node_kentr/config
.
├── client_local_threshold.toml
└── client_remote_threshold.toml
</code></pre>
<ul>
<li>actual kentr_non-admin (<a href="https://github.com/omystik/omystik/blob/main/osatcon/headquarter/config">osatcon/headquarter/config</a>)</li>
</ul>
<pre><code>.
├── client_local_threshold.toml
└── client_remote_threshold.toml
</code></pre>
<h2 id="3-nodes-cluster-configuration">3. Nodes Cluster configuration</h2>
<p>Every node reads a TOML cluster file from <code>meshes_config/</code> and selects its own entry by
<code>--&lt;role&gt;-id</code>. Three variants exist per cluster:</p>
<table>
<thead>
<tr>
<th>Variant</th>
<th>Use</th>
</tr>
</thead>
<tbody><tr>
<td><code>*_local.toml</code></td>
<td>Everything on <code>127.0.0.1</code>, start here</td>
</tr>
<tr>
<td><code>*_test.toml</code></td>
<td>Mixed local and remote</td>
</tr>
<tr>
<td><code>*.toml</code></td>
<td>Production addresses</td>
</tr>
</tbody></table>
<pre><code>mesh_a_cluster_local.toml     Mesh A: syndesmos + autonomos
mesh_c_cluster_local.toml     Mesh C: ypolo executors
mpc_cluster_local.toml        Mesh B: kryphos MPC parties&#39; face B
gateway_cluster_local.toml    Pylon gateways (Face A + Face B)
kentr_cluster_local.toml      Kentr control nodes
</code></pre>
<blockquote class="docs-alert docs-alert-caution"><p class="docs-alert-title">Caution</p><p>Committed configs assign each node a one-byte <code>secret_seed</code> from which its keypair is
derived. Treat every peer identity in these files as public. Never reuse them outside a
local test. See <a href="/security/#deterministic-node-identities-from-short-seeds">SECURITY.md</a>.</p>
</blockquote>
<h3 id="the-pylons-rendezvous-addresses-are-in-rust-not-in-toml">The Pylon&#39;s rendezvous addresses are in Rust, not in TOML</h3>
<p>This one will cost you an afternoon if you meet it by accident.</p>
<p>The gateway configuration has a <code>rendezvous</code> field listing the Syndesmos nodes the Pylon
should register with. <strong>None of the three shipped <code>gateway_cluster*.toml</code> files sets it.</strong>
The field is declared with a serde default:</p>
<pre><code class="language-rust">// meshes_config/src/gateway_config.rs
#[serde(default = &quot;default_rendezvous_bootstraps_de&quot;)]
pub rendezvous: Vec&lt;RendezvousABootstrap&gt;,
</code></pre>
<p>so when the TOML omits it, the Pylon falls back to the values hardcoded in
<code>default_rendezvous_bootstraps()</code> in that same file. In every shipped configuration, that
function is therefore the only place the Pylon learns where Syndesmos is.</p>
<p>Those hardcoded values must match the <code>[[syndesmos]]</code> entries of the Mesh A cluster file you
are actually running. Out of the box they mirror <code>mesh_a_cluster_local.toml</code>:</p>
<table>
<thead>
<tr>
<th></th>
<th><code>gateway_config.rs</code> default</th>
<th><code>mesh_a_cluster_local.toml</code></th>
</tr>
</thead>
<tbody><tr>
<td>Syndesmos 1</td>
<td><code>/ip4/127.0.0.1/udp/9008/quic-v1</code></td>
<td><code>[[syndesmos]]</code> id 1, same address and peer id</td>
</tr>
<tr>
<td>Syndesmos 2</td>
<td><code>/ip4/127.0.0.1/udp/9002/quic-v1</code></td>
<td><code>[[syndesmos]]</code> id 2, same address and peer id</td>
</tr>
</tbody></table>
<p>So a purely local run works unchanged. <strong>Switching to <code>_test</code> or production addresses does
not.</strong> Change the Syndesmos address in the cluster TOML and the Pylon will keep dialling
<code>127.0.0.1</code>, register with nothing, and the mesh will look healthy while no node ever finds
the gateway.</p>
<p>Two ways out, either is fine:</p>
<ul>
<li><strong>Set <code>rendezvous</code> in your gateway TOML.</strong> The field exists; filling it makes the TOML
authoritative and the Rust default irrelevant. This is the better habit.</li>
<li><strong>Edit <code>default_rendezvous_bootstraps()</code></strong> in <code>meshes_config/src/gateway_config.rs</code> to
match, and rebuild. <code>mesh_a_cluster_local.toml</code> carries a reminder comment on the
<code>listen_addr</code> line for exactly this reason.</li>
</ul>
<p>Both the address and the peer id have to agree. A matching address with a stale peer id fails
the same way, and more confusingly, because the connection succeeds and the identity check
does not.</p>
<p>This hardcode will disappear when the mesh of gateway nodes (mesh-p) will be developed.</p>
<h2 id="4-storage-environment">4. Storage environment</h2>
<p>A storage node locates its data through environment variables. Set them <strong>per shell</strong>,
before launching a node. Each node needs its own tree
(see <a href="/docs/node-layout/">NODE_LAYOUT.md</a>):</p>
<pre><code class="language-bash">export STORAGE_DIR=&quot;./nodeA/content/&quot;
export STORAGE_MIDCID=&quot;./nodeA/metadata/midcid/&quot;
export STORAGE_MERKLE=&quot;./nodeA/metadata/metamerkle/&quot;
export STORAGE_METADATA=&quot;./nodeA/metadata/metamerkle/&quot;
export BLOB_STORAGE_DIR=&quot;./nodeA/blob/&quot;
</code></pre>
<p>To clear them:</p>
<pre><code class="language-bash">unset STORAGE_DIR STORAGE_MIDCID STORAGE_MERKLE STORAGE_METADATA BLOB_STORAGE_DIR
</code></pre>
<h2 id="5-mesh-a-only-encrypted-storage">5. Mesh A only: encrypted storage</h2>
<p>The smallest useful setup: a rendezvous node and a peer exchanging encrypted files. No FHE
involved.</p>
<p><strong>Terminal 1, rendezvous (<code>syndesmos</code>)</strong></p>
<pre><code class="language-bash">cd node_syndesmos
cargo run -- --mesh-a-cluster ../meshes_config/mesh_a_cluster_local.toml --syndesmos-id 1
</code></pre>
<p>if needed
<code>RUST_LOG=libp2p_swarm=debug,libp2p_quic=debug,libp2p_rendezvous=debug</code></p>
<p><strong>Terminal 2, peer (<code>autonomos</code>)</strong>, with its own storage environment exported</p>
<pre><code class="language-bash">cd node_autonomos
cargo run -- --mesh-a-cluster ../meshes_config/mesh_a_cluster_local.toml --autonomos-id 1
</code></pre>
<p><strong>Terminal n, peer (either <code>syndesmos</code>/<code>autonomos</code>)</strong>, with its own storage environment exported
for instance:</p>
<pre><code class="language-bash">cd node_autonomos
cargo run -- --mesh-a-cluster ../meshes_config/mesh_a_cluster_local.toml --autonomos-id n
</code></pre>
<p>They expose an interactive prompt:</p>
<pre><code>store &lt;file_name&gt; &lt;file_type&gt; &lt;secret&gt; &lt;file_path&gt;
call  &lt;file_name&gt; &lt;file_type&gt; &lt;secret&gt;
get   &lt;file_name&gt; &lt;file_type&gt; &lt;secret&gt; &lt;private_key&gt; &lt;nonce&gt;
</code></pre>
<ul>
<li><strong><code>store</code></strong> encrypts the file, computes its MID and CID, and publishes the MID to the
DHT. It prints the <strong>key and nonce. Record them, the mesh does not keep a copy.</strong></li>
<li><strong><code>call</code></strong> recomputes the MID from the three metadata values and locates providers and stores
into the caller content and metadata folders and merkle tree.</li>
<li><strong><code>get</code></strong> retrieves and decrypts, given the key and nonce, it will be located at <code>./data/decrypt_content</code>.</li>
</ul>
<pre><code>store briefing .pdf 8f3kQ9zR2vLmXw ./samples/briefing.pdf
call  briefing .pdf 8f3kQ9zR2vLmXw
get   briefing .pdf 8f3kQ9zR2vLmXw &lt;key-hex&gt; &lt;nonce-hex&gt;
</code></pre>
<p>Use a high-entropy <code>secret</code>: anyone who can guess all three metadata values can locate the
content.</p>
<h2 id="6-full-stack-confidential-compute">6. Full stack: confidential compute</h2>
<p>Bring the tiers up in order. Each needs its own terminal.</p>
<p><strong>Mesh B comes first.</strong> Key material has to exist before a party will start, and the
parties have to be running and initialized before the gateway or an executor can do
anything. The order in §6.1 – §6.4 is not a suggestion.</p>
<h3 id="61-generate-kms-key-material-once-before-anything-else">6.1 Generate KMS key material (once, before anything else)</h3>
<p>Run from <code>node_kryphos/</code>, so the key and certificate directories land where the party
configuration expects them:</p>
<pre><code class="language-bash">cd node_kryphos
</code></pre>
<p>Generate the threshold key material, meaning signing keys, FHE key shares and the CRS:</p>
<pre><code class="language-bash">cargo run -p kms --bin kms-gen-keys -F testing -F threshold-fhe/testing -- \\
  --private-storage file --private-file-path ./keys \\
  --public-storage  file --public-file-path  ./keys \\
  threshold
</code></pre>
<p><code>--cmd</code> narrows what is produced to <code>all</code> (the default), <code>signing-keys</code>, <code>fhe-keys</code> or
<code>crs</code>, which is useful when regenerating one piece. <code>threshold</code> is the mode; <code>centralized</code>
exists for a single-party KMS that ØMYSTIK does not use.</p>
<blockquote class="docs-alert docs-alert-note"><p class="docs-alert-title">Note</p><p>Threshold mode generates the key shares <em><strong>centrally</strong></em> and then distributes them. That is
a testing convenience, not the threshold ceremony: for a deployment where the threshold
guarantee has to hold, key generation runs as an MPC protocol through Kentr node
(§6.7), not through this command.</p>
</blockquote>
<h3 id="62-generate-tls-certificates-once">6.2 Generate TLS certificates (once)</h3>
<p>The MPC cores authenticate each other with TLS. Generate one CA per party. <code>--ca-count</code>
must match the number of parties in your <code>mpc_cluster_*.toml</code>:</p>
<pre><code class="language-bash">cargo run -p kms --bin kms-gen-tls-certs -- --ca-prefix p --ca-count 4
</code></pre>
<h3 id="63-mesh-b-start-the-kryphos-parties">6.3 Mesh B: start the Kryphos parties</h3>
<p>One terminal per party. Repeat with <code>--node-id 2</code>, <code>3</code>, <code>4</code>, matching the <code>[[node]]</code>
entries in <code>mpc_cluster_local.toml</code>:</p>
<pre><code class="language-bash">cd node_kryphos
RUST_LOG=&quot;info,kms_threshold=debug,kms_lib=debug,threshold_fhe=debug&quot; \\
  cargo run --bin node_fhe --features &quot;p2p insecure&quot; -- \\
  --kryphos-config ../meshes_config/mpc_cluster_local.toml --node-id 1
</code></pre>
<p><code>node_fhe</code> <strong>is</strong> the KMS server here. It calls the same <code>run_server</code> entry point upstream
runs as <code>kms-server</code>, wrapped so that the inter-party transport is libp2p.</p>
<p>Each party ends up listening on three things, which is worth knowing when a port looks
wrong:</p>
<table>
<thead>
<tr>
<th>Port</th>
<th>From</th>
<th>Carries</th>
</tr>
</thead>
<tbody><tr>
<td><code>50100</code>, <code>50200</code>, … (TCP)</td>
<td><code>[service] listen_port</code> in <code>kms/core/service/config/default_N.toml</code></td>
<td>the gRPC service, which is what <code>kms-init</code> and the core client talk to</td>
</tr>
<tr>
<td><code>50100</code>, <code>50200</code>, … (UDP/QUIC)</td>
<td><code>listen_address</code> in <code>mpc_cluster_*.toml</code></td>
<td>libp2p MPC rounds between parties</td>
</tr>
<tr>
<td><code>8100</code>, <code>8200</code>, … (UDP/QUIC)</td>
<td><code>gateway_listen_addr</code> in <code>mpc_cluster_*.toml</code></td>
<td>Face B, from the Pylon</td>
</tr>
</tbody></table>
<p>The first two share a number but not a transport, so they do not collide.</p>
<p>Wait until the parties report each other connected before continuing.</p>
<h3 id="64-initialise-the-parties-once-per-set-of-nodes">6.4 Initialise the parties (once per set of nodes)</h3>
<p>With all parties up, initialise them. The addresses are the <strong>gRPC service</strong> ports:</p>
<pre><code class="language-bash">cargo run -p kms --bin kms-init -- \\
  -a http://127.0.0.1:50100 \\
     http://127.0.0.1:50200 \\
     http://127.0.0.1:50300 \\
     http://127.0.0.1:50400
</code></pre>
<p>This is the step most easily got wrong, so four facts about it:</p>
<ul>
<li><strong>Run it exactly once</strong> for a given set of parties. Calling it again returns an error.</li>
<li>The material it produces is written to each party&#39;s private storage under
<code>PRIV-p&lt;N&gt;/PrssSetup/</code>.</li>
<li><strong>Restarting a party does not need it again.</strong> A party finds the material on disk and
reuses it, which is how a failed node rejoins an existing set.</li>
<li><strong>Changing the set of parties does.</strong> A different number of parties, or different
parties, needs fresh init material, and the only way to get it is to delete
<code>PRIV-p&lt;N&gt;/PrssSetup/</code> on every party by hand and run <code>kms-init</code> again.</li>
</ul>
<p>Until this succeeds, public and user decryption will not work, however healthy the parties
look.</p>
<h3 id="65-pylon-gateway">6.5 Pylon gateway</h3>
<pre><code class="language-bash">cd node_pylon
RUST_LOG=info cargo run --bin node_pylon -- \\
  --gateway-cluster ../meshes_config/gateway_cluster_local.toml \\
  --mpc-cluster     ../meshes_config/mpc_cluster_local.toml \\
  --gateway-id 1
</code></pre>
<p>Narrower logging for gateway internals: <code>RUST_LOG=libp2p_swarm=debug,libp2p_quic=debug,libp2p_rendezvous=debug</code></p>
<h3 id="66-mesh-c-ypolo-executor">6.6 Mesh C: Ypolo executor</h3>
<pre><code class="language-bash">cd node_ypolo
RUST_LOG=libp2p_swarm=debug,libp2p_rendezvous=debug \\
  cargo run --bin ypolo -- \\
  --ypolo-cluster ../meshes_config/mesh_c_cluster_local.toml --ypolo-id 1
</code></pre>
<h3 id="67-kentr-keys-and-kms-operations">6.7 Kentr: keys and KMS operations</h3>
<p>Fetch and rehydrate an existing keyset:</p>
<pre><code class="language-bash">cd node_kentr
cargo run --bin kentr -- \\
  --kentr-cluster ../meshes_config/kentr_cluster_local.toml \\
  --kentr-id 1 \\
  --rehydrate-key \\
  -- -f config/client_local_threshold.toml
</code></pre>
<p>Arguments after <code>--</code> pass through to Zama&#39;s core client. Key generation is two-phase:
preprocessing first, then generation with the resulting id:</p>
<p>Preproc and KeyGen:</p>
<pre><code class="language-bash"># 1. preprocessing, note the PREPROC_ID it returns
cargo run --bin kentr -- \\
  --kentr-cluster ../meshes_config/kentr_cluster_local.toml --kentr-id 1 \\
  -- -f config/client_local_threshold.toml -a -l preproc-key-gen

# 2. generation
cargo run --bin kentr -- \\
  --kentr-cluster ../meshes_config/kentr_cluster_local.toml --kentr-id 1 \\
  -- -f config/client_local_threshold.toml -a -l key-gen --preproc-id &lt;PREPROC_ID&gt;

# NB
For convenience within 0MYSTIK, these commands have been merged into a single-shot generation: with \`-l key-gen-fresh\`.
cargo run --bin kentr -- --kentr-cluster ../meshes_config/kentr_cluster_local.toml --kentr-id 1 -- -f config/client_local_threshold.toml -a -l key-gen-fresh

#[!Warning]
At client_xxx_threshold.toml, the fhe_params parameters impacts the keys generation time:
- &quot;Test&quot;, Small, insecure parameters for testing takes &quot;around 27 to 35 minutes&quot;.
- &quot;Default&quot;, Large, secure parameters takes at &quot;least 50 hours&quot;.
</code></pre>
<p>CRS generation:</p>
<pre><code class="language-bash">cargo run --bin kentr -- \\
  --kentr-cluster ../meshes_config/kentr_cluster_local.toml --kentr-id 1 \\
  -- -f config/client_local_threshold.toml -a -l crs-gen --max-num-bits 2048
</code></pre>
<p>Round-trip check, encrypting a value and decrypting it through the threshold:</p>
<pre><code class="language-bash">cargo run --bin kentr -- \\
  --kentr-cluster ../meshes_config/kentr_cluster_local.toml --kentr-id 1 \\
  -- -f config/client_local_threshold.toml -a -l user-decrypt from-args \\
     --to-encrypt 0x2342 --data-type euint16 --key-id &lt;KEY_ID&gt;
</code></pre>
<hr>
<p><br><br></p>
<h1 id="osatcon">OSATCON</h1>
<h2 id="1-running-demo">1. Running demo</h2>
<p>The reference application, on top of a running full stack (<a href="#6">§6</a>).</p>
<p><strong>Install the smart program</strong> on the executor. <code>--runner-command</code> is the absolute path to
the built <code>smart-program-alert</code> binary on the machine hosting Ypolo:</p>
<pre><code class="language-bash">cargo build -p smart-program-alert --bin smart-program-alert
cargo build -p smart-program-alert --bin deploy_alert

cd osatcon/smart-program-alert
cargo run --bin deploy_alert -- \\ 
  --kentr-cluster ../../meshes_config/kentr_cluster.toml \\
  --kentr-id 2 \\
  --ypolo-cluster ../../meshes_config/mesh_c_cluster.toml \\
  --ypolo-id 1 \\
  --runner-command /YOUR_PATH/mesh/target/debug/smart-program-alert
</code></pre>
<p><strong>NB</strong>: for some reasons, this command may failed if not one line.</p>
<p><strong>Field terminals.</strong> All participants must share the same <code>--session-code</code>:</p>
<pre><code class="language-bash"># convoy
cd osatcon/edge-convoy-operator
cargo run -p edge-convoy-operator -- \\
  --mesh-a-cluster ../../meshes_config/mesh_a_cluster_local.toml --autonomos-id 1 \\
  --ypolo-cluster  ../../meshes_config/mesh_c_cluster_local.toml --ypolo-id 1 \\
  --session-code &#39;&lt;your_choice&gt;&#39;

# satellite
cd osatcon/edge-satellite-operator
cargo run -p edge-satellite-operator -- \\
  --mesh-a-cluster ../../meshes_config/mesh_a_cluster_local.toml --autonomos-id 2 \\
  --ypolo-cluster  ../../meshes_config/mesh_c_cluster_local.toml --ypolo-id 1 \\
  --session-code &#39;&lt;your_choice&gt;&#39;
</code></pre>
<p><strong>Command view:</strong></p>
<pre><code class="language-bash">cd osatcon/headquarter
cargo run -- \\
  --kentr-cluster ../../meshes_config/kentr_cluster_local.toml --kentr-id 2 \\
  --ypolo-cluster ../../meshes_config/mesh_c_cluster_local.toml --ypolo-id 1 \\
  --session-code &#39;&lt;your_choice&gt;&#39;
  -- -f config/client_local_threshold.toml
</code></pre>
<p>Add <code>--ui-only</code> to inspect the interface without a live mesh.</p>
<br><h2 id="2-cross-compiling-for-edge-hardware">2. Cross-compiling for edge hardware</h2>
<h3 id="current-crosstoml"><a href="https://github.com/omystik/omystik/blob/main/Cross.toml">Current cross.toml</a></h3>
<p>The reference deployment deliberately targets old and constrained machines.</p>
<p><strong>Raspberry Pi Zero 2 W</strong> (armv7) for <code>syndesmos</code> and <code>pylon</code>:</p>
<pre><code class="language-bash">CARGO_TARGET_DIR=target-pi cross build --release \\
  --target armv7-unknown-linux-gnueabihf \\
  -p node_syndesmos --bin node_syndesmos
</code></pre>
<p><strong>Raspberry Pi 5</strong> (aarch64) for the convoy operator:</p>
<pre><code class="language-bash">cross build -p edge-convoy-operator \\
  --target aarch64-unknown-linux-gnu \\
  --profile release-lto-off --no-default-features -j 1
</code></pre>
<p><strong>x86_64 Linux</strong> for the satellite operator:</p>
<pre><code class="language-bash">cross build -p edge-satellite-operator \\
  --target x86_64-unknown-linux-gnu \\
  --no-default-features --release -j 1
</code></pre>
<p><strong>Older macOS</strong> (10.13) for ypolo and the alert program:</p>
<pre><code class="language-bash">MACOSX_DEPLOYMENT_TARGET=10.13 cargo build -p node_ypolo \\
  --bin ypolo --release --target x86_64-apple-darwin
MACOSX_DEPLOYMENT_TARGET=10.13 cargo build -p smart-program-alert \\
  --bin smart-program-alert --release --target x86_64-apple-darwin
</code></pre>
<p><code>release-lto-off</code> and <code>-j 1</code> exist because full LTO exhausts memory on constrained build
hosts. <code>Cross.toml</code> holds the per-target settings.</p>
<p>Deployed binaries take the same arguments, with configs alongside:</p>
<pre><code class="language-bash">./node_syndesmos --mesh-a-cluster mesh_a_cluster.toml --syndesmos-id 1
DISPLAY=:0 ./edge-convoy-operator \\
  --mesh-a-cluster mesh_a_cluster.toml --autonomos-id 1 \\
  --ypolo-cluster mesh_c_cluster.toml --ypolo-id 1 --session-code &#39;&lt;your_choice&gt;&#39;
</code></pre>
<br><h1 id="troubleshooting">Troubleshooting</h1>
<p><strong><code>Failed to dial … quic-v1</code></strong>: the target isn&#39;t listening, or its address/peer ID in the
cluster file is stale. Rendezvous nodes must be up before anything registers.</p>
<p><strong>Face B requests fail or hang</strong>: the MPC cluster is incomplete. All parties named in
<code>mpc_cluster_local.toml</code> must be running and mutually connected; the gateway&#39;s
<code>num_reconstruct</code> cannot be satisfied otherwise.</p>
<p><strong><code>get</code> fails to decrypt</strong>: key and nonce must match the ones <code>store</code> printed. They are
not recoverable from the mesh.</p>
<p><strong><code>call</code> finds no providers</strong>: either the metadata triple doesn&#39;t match what was stored
(all three fields, exactly), or the MID has not yet propagated to the DHT.</p>
<p><strong>Storage paths wrong or shared</strong>: each node needs its own <code>STORAGE_*</code> environment. Two
nodes pointed at one tree will corrupt each other&#39;s state.</p>
<p>Useful log filters:</p>
<pre><code class="language-bash">RUST_LOG=libp2p_swarm=debug,libp2p_quic=debug,libp2p_rendezvous=debug   # networking
RUST_LOG=&quot;info,kms_lib=debug,threshold_fhe=debug&quot;                        # KMS / MPC
RUST_LOG=faceb=info,node_pylon::runtime=info                             # gateway
</code></pre>
<hr>
<p><em>Architecture: <a href="/docs/architecture/">ARCHITECTURE.md</a> · Crates: <a href="/docs/crates/">CRATES.md</a> ·
Node data tree: <a href="/docs/node-layout/">NODE_LAYOUT.md</a></em></p>
`},{route:`/docs/lineage/`,source:`docs/LINEAGE.md`,title:`Lineage`,html:`<h1 id="lineage">Lineage</h1>
<p>Where ØMYSTIK came from, and which parts of it are inherited rather than new.</p>
<p>ØMYSTIK is not a single project that grew in one line. It draws on three earlier projects by
the same author, developed independently of one another, each addressing a different problem.
Two of them are implemented in the codebase today. The third is the origin of work that
remains on the roadmap, and this document is explicit about which is which.</p>
<p>For what the system does now, read <a href="/docs/architecture/">ARCHITECTURE.md</a>. For what is and is not
implemented, see the &quot;What works, and what doesn&#39;t&quot; section of the
<a href="/readme/">README</a>.</p>
<hr>
<h2 id="three-lineages">Three lineages</h2>
<pre><code>                              ØMYSTIK
                                 ▲
                                 │
          ┌──────────────────────┼──────────────────────┐
          │                      │                      │
    MYSTIK&gt;p2p              FHE.Chess               FHE.IRM
          │                      │                      │
  persistent encrypted    deep learning in FHE    blockchain in FHE
     p2p storage          Concrete ML, TFHE       fhEVM, KMS, TFHE
       MID / CID          quantized CNN           encrypted-data operations
   content discovery      homomorphic inference   rights management
</code></pre>
<p>None of the three derives from another. They converge in ØMYSTIK rather than succeed one
another.</p>
<hr>
<h2 id="mystikp2p">MYSTIK&gt;p2p</h2>
<p><em>November 2024 to March 2025. Implemented in ØMYSTIK today.</em></p>
<p>A persistent encrypted peer-to-peer storage network. Files are encrypted before they touch
disk and addressed by a content identifier derived from the ciphertext. Metadata is hashed
into a Merkle tree whose root serves as a lookup identifier, so a node can advertise what it
holds without publishing what it is.</p>
<p>This is the most directly inherited of the three. The storage model is Mesh A today: the same
CID and MID construction, the same two Merkle layers, the same principle that the network
never holds the decryption key.</p>
<p>The original project description is kept in this repository at
<a href="/readme-mystik-p2p/">README_mystik-p2p.md</a>, because it remains the clearest account of
the storage model.</p>
<hr>
<h2 id="fhechess">FHE.Chess</h2>
<p><em>2023. Not implemented in ØMYSTIK. Origin of a roadmap item.</em></p>
<p>Repository: <a href="https://github.com/vrona/FHE.Chess">https://github.com/vrona/FHE.Chess</a></p>
<p>A chess application whose AI opponent infers over a board it cannot read. Move prediction is
carried by two convolutional neural networks, Source and Target, trained in PyTorch, then
retrained quantization-aware with Brevitas at 4-bit weights and activations, then compiled
with Zama&#39;s Concrete ML so that inference executes homomorphically. The application exposes
three modes, <code>clear</code>, <code>simfhe</code> and <code>deepfhe</code>, the last performing the inference while the
data stays encrypted. It was produced in answer to a Zama bounty.</p>
<p><strong>ØMYSTIK does not contain this capability.</strong> Deep FHE inference and federated learning are
roadmap items, not code. FHE.Chess is where that work was actually done, in a different
shape and on a different problem, and it is cited here as the antecedent of an intended
direction rather than as a component of the current system.</p>
<p>It is also the only one of the three that has always been public, so its history can be
inspected independently.</p>
<hr>
<h2 id="fheirm">FHE.IRM</h2>
<p><em>Spring 2024. Partly implemented in ØMYSTIK.</em></p>
<p>A decentralized application managing access rights and updates to confidential documents on
Ethereum, built on Zama&#39;s fhEVM, including its key management and TFHE. Where FHE.Chess
applied homomorphic encryption to model inference, FHE.IRM applied it to rights management:
operations over encrypted data, encrypted metadata, and authorization workflows that never
expose the underlying document.</p>
<p>What carries into ØMYSTIK is the encrypted-computation model and the use of threshold key
management, now running over a peer-to-peer mesh instead of a blockchain. The on-chain
rights-management layer does not carry over; ØMYSTIK has no blockchain component.</p>
<hr>
<h2 id="what-is-inherited-and-what-is-not">What is inherited and what is not</h2>
<table>
<thead>
<tr>
<th>From</th>
<th>Inherited</th>
<th>Status in ØMYSTIK</th>
</tr>
</thead>
<tbody><tr>
<td>MYSTIK&gt;p2p</td>
<td>Encrypted storage, CID/MID addressing, content discovery</td>
<td>Implemented, Mesh A</td>
</tr>
<tr>
<td>FHE.IRM</td>
<td>Computation over encrypted data, threshold key custody</td>
<td>Implemented, Mesh B and Mesh C</td>
</tr>
<tr>
<td>FHE.IRM</td>
<td>On-chain rights management</td>
<td>Not carried over</td>
</tr>
<tr>
<td>FHE.Chess</td>
<td>Deep learning under FHE, quantized model inference</td>
<td>Not implemented, roadmap</td>
</tr>
</tbody></table>
<hr>
<h2 id="external-context">External context</h2>
<p>On 2 June 2025, NATO&#39;s Defence Innovation Accelerator for the North Atlantic (DIANA)
published its 2026 Advanced Communication Technologies challenge. That challenge
independently identifies several technical areas overlapping the direction of ØMYSTIK:
resilient and decentralised communications, packet-switched mesh networks, edge computing,
homomorphic encryption, and technologies intended for both civilian and military
environments.</p>
<p>It is cited here as external context on the problem space, nothing more. It does not imply
that NATO or DIANA has evaluated, validated, endorsed or approved ØMYSTIK, and no
application, submission or relationship of any kind is claimed.</p>
<hr>
<h2 id="third-party-provenance">Third-party provenance</h2>
<p>The cryptography in all three projects comes from <a href="https://www.zama.ai/">Zama</a>: Concrete ML
in FHE.Chess, fhEVM in FHE.IRM, TFHE-rs and the KMS in ØMYSTIK. The KMS is vendored into
this repository and modified, which is documented in <a href="https://github.com/omystik/omystik/blob/main/kms/UPSTREAM.md">kms/UPSTREAM.md</a>.</p>
<p>Licence terms, and the Zama commercial-use obligation that applies to anyone building on
this, are in <a href="/licensing/">LICENSING.md</a>.</p>
`},{route:`/docs/node-layout/`,source:`docs/NODE_LAYOUT.md`,title:`Node Runtime Layout`,html:`<h1 id="node-runtime-layout">Node Runtime Layout</h1>
<p>A ØMYSTIK node keeps its state on disk in a specific directory tree. This document
describes that tree, what creates each part, and how nodes are kept from colliding.</p>
<hr>
<h2 id="1-storage-node-tree">1. Storage node tree</h2>
<blockquote>
<p><code>nodeA/</code> and <code>nodeN/</code> at the repository root <strong>are not crates.</strong> They are worked examples
of this tree, one per node. When running several nodes on one machine, use them and
create more as needed.</p>
</blockquote>
<p>A node that stores encrypted content needs this structure to exist:</p>
<pre><code>nodeA/
├── content/                 encrypted files, named by CID  (STORAGE_DIR)
├── metadata/
│   ├── metamerkle/          classic Merkle trees per MID   (STORAGE_MERKLE, STORAGE_METADATA)
│   └── midcid/              sparse Merkle tree: MID → CID  (STORAGE_MIDCID)
└── blob/                    Face A blob staging            (BLOB_STORAGE_DIR)
    ├── acks/                pinned-blob acknowledgements
    └── pubkeys/             fetched FHE key artifacts
</code></pre>
<p>Nothing creates this tree for you at first run. <strong>Create it before starting a node</strong>, or
the node will fail on paths that don&#39;t exist.</p>
<pre><code class="language-bash">mkdir -p nodeA/{content,metadata/{metamerkle,midcid},blob/{acks,pubkeys}}
</code></pre>
<h2 id="2-environment-variables">2. Environment variables</h2>
<p>The tree is located entirely through the environment. There is no config-file equivalent:
if a variable is unset, the code falls back to a default under <code>./data/</code>.</p>
<table>
<thead>
<tr>
<th>Variable</th>
<th>Points at</th>
<th>Default</th>
<th>Read by</th>
</tr>
</thead>
<tbody><tr>
<td><code>STORAGE_DIR</code></td>
<td><code>content/</code></td>
<td><code>./data/content/</code></td>
<td><code>content_hashing::get_storage_dir</code></td>
</tr>
<tr>
<td><code>STORAGE_MERKLE</code></td>
<td><code>metadata/metamerkle/</code></td>
<td><code>./data/metadata/metamerkle/</code></td>
<td><code>metadata_mrkl::get_storage_merkle</code></td>
</tr>
<tr>
<td><code>STORAGE_MIDCID</code></td>
<td><code>metadata/midcid/</code></td>
<td><code>./data/metadata/midcid/</code></td>
<td><code>metadata_mrkl::get_storage_midcid</code></td>
</tr>
<tr>
<td><code>STORAGE_METADATA</code></td>
<td><code>metadata/metamerkle/</code></td>
<td>none</td>
<td>node binaries</td>
</tr>
<tr>
<td><code>BLOB_STORAGE_DIR</code></td>
<td><code>blob/</code></td>
<td><code>./data/blob/</code></td>
<td><code>libp2p_common::komvos::get_storage_blob</code>, <code>ypolo</code></td>
</tr>
<tr>
<td><code>KEYS_DIR</code></td>
<td>key material</td>
<td><code>./data/core/keys/</code></td>
<td><code>content_hashing::get_keys_dir</code></td>
</tr>
</tbody></table>
<p>Set them per shell, before launching each node:</p>
<pre><code class="language-bash">export STORAGE_DIR=&quot;./nodeA/content/&quot;
export STORAGE_MIDCID=&quot;./nodeA/metadata/midcid/&quot;
export STORAGE_MERKLE=&quot;./nodeA/metadata/metamerkle/&quot;
export STORAGE_METADATA=&quot;./nodeA/metadata/metamerkle/&quot;
export BLOB_STORAGE_DIR=&quot;./nodeA/blob/&quot;
</code></pre>
<blockquote class="docs-alert docs-alert-caution"><p class="docs-alert-title">Caution</p><p><strong>Two nodes must never share one tree.</strong> The sparse Merkle tree uses an embedded
key-value store (<code>sled</code>) that assumes a single writer; two processes pointed at the same
<code>midcid/</code> will corrupt each other&#39;s state. One tree per node, always. This is the whole
reason <code>nodeA</code> … <code>nodeN</code> exist as separate directories.</p>
</blockquote>
<p>Because the paths are relative by default, they resolve against each node&#39;s <strong>working
directory</strong>. Running <code>cargo run</code> from inside <code>node_autonomos/</code> while exporting
<code>./nodeA/content/</code> resolves to <code>node_autonomos/nodeA/content/</code>, not the repository root.
Use absolute paths if that ambiguity bites.</p>
<h2 id="3-what-lives-where">3. What lives where</h2>
<h3 id="content-encrypted-content"><code>content/</code>: encrypted content</h3>
<p>Ciphertexts produced by <code>content-hashing</code>, each named by its <strong>CID</strong> with a <code>.bin</code>
extension. Written by <code>encrypt_file</code>, read by <code>decrypt_file</code>. Plaintext never appears here;
the encryption is streaming, so the plaintext is never fully in memory either.</p>
<h3 id="metadatametamerkle-metadata-merkle-trees"><code>metadata/metamerkle/</code>: metadata Merkle trees</h3>
<p>One subdirectory per <strong>MID</strong>, holding the persisted classic Merkle tree (<code>rs_merkle</code>,
SHA-256) whose leaves are the file&#39;s <code>file_name</code>, <code>file_type</code>, and <code>secret</code>. Presence of a
MID directory is what lets a node prove it holds metadata matching a query without
revealing the metadata.</p>
<p><code>db-access</code> reconstructs MID/CID pairs by reading directory names here against <code>.bin</code> names
in <code>content/</code>, a recovery path when the sparse tree is lost (when the node is switched off,
for instance).</p>
<h3 id="metadatamidcid-the-mid--cid-map"><code>metadata/midcid/</code>: the MID → CID map</h3>
<p>The sparse Merkle tree (<code>monotree</code>, Blake3 hashing, <code>sled</code> backend) mapping each MID to its
CID. This is the authoritative index: given a MID, it yields the ciphertext to serve. The
pairing is described in <a href="/readme-mystik-p2p/">README_mystik-p2p.md</a>, which documents the
original MYSTIK&gt;p2p storage model.</p>
<h3 id="blob-face-a-staging"><code>blob/</code>: Face A staging</h3>
<p>Blobs transferred over <code>/mesh/gateway/blob/1</code>, addressed by a Blake3-derived <code>BlobId</code>.
Compute inputs too large to inline in a <code>ComputeJobSpec</code> land here on their way to an
executor.</p>
<ul>
<li><strong><code>acks/</code></strong> holds acknowledgements that a gateway has pinned a blob (<code>Client::ack_pinned</code>),
so a client knows its upload is durable before submitting the job that depends on it.</li>
<li><strong><code>pubkeys/</code></strong> holds FHE key artifacts, in the layout <code>key_ops</code> expects:
<code>PUB-p&lt;N&gt;/&lt;KeyType&gt;/&lt;key_id&gt;</code>, where <code>KeyType</code> is <code>PublicKey</code>, <code>PublicKeyMetadata</code>, or
<code>ServerKey</code>. They arrive either from the Pylon via Face A <code>GetArtifact</code> (a Kentr running
<code>--rehydrate-key</code>) or as a replica pulled from a peer over <code>/key/1</code>; never from the DHT.
<code>unique_public_key_id_from_pub_folders</code> walks every <code>PUB-p*</code> directory and confirms all
parties report the same key id. A mismatch means the node holds an inconsistent keyset
and must re-fetch.</li>
</ul>
<h2 id="4-gateway-store-a-different-tree">4. Gateway store: a different tree</h2>
<p><code>node_pylon</code> does not use the node tree above. It keeps a <code>JobStore</code> under the <code>data_dir</code>
from its cluster config (<code>gateway.face_a.data_dir</code>, e.g. <code>./data/gateway</code>), created on
first open:</p>
<pre><code>data/gateway/
├── jobs/                    job records and state
├── blobs/                   blob content
├── acks/                    pin acknowledgements
├── programs/                installed program packages
├── program_refs/            program id/version references
├── artifact_aliases/        named aliases → artifact ids
└── artifact_manifests/      signed artifact manifests
</code></pre>
<p>Unlike the node tree, <code>JobStore::open</code> creates all of these itself.</p>
<p><code>node_ypolo</code> similarly keeps program and artifact state under its own data directory, plus
<code>BLOB_STORAGE_DIR</code> for staged inputs.</p>
<h2 id="5-which-directories-are-disposable">5. Which directories are disposable</h2>
<p>Useful when copying, backing up, or packaging a repository.</p>
<table>
<thead>
<tr>
<th>Directory</th>
<th>Disposable?</th>
<th></th>
</tr>
</thead>
<tbody><tr>
<td><code>target/</code>, <code>target-pi/</code></td>
<td>Yes</td>
<td>Build output</td>
</tr>
<tr>
<td><code>jobs/</code>, <code>blobs/</code>, <code>artifacts/</code></td>
<td>Yes</td>
<td>Gateway/executor runtime state; rebuilt on demand</td>
</tr>
<tr>
<td><code>content/</code></td>
<td><strong>No</strong></td>
<td>Encrypted content; losing it loses the data</td>
</tr>
<tr>
<td><code>metadata/</code></td>
<td><strong>No</strong></td>
<td>Losing <code>midcid/</code> orphans content; <code>metamerkle/</code> can partly rebuild it via <code>db-access</code></td>
</tr>
<tr>
<td><code>blob/pubkeys/</code></td>
<td>Yes</td>
<td>Re-fetchable from the mesh</td>
</tr>
<tr>
<td><code>keys/</code>, <code>certs/</code>, <code>backup_vault/</code></td>
<td><strong>No</strong></td>
<td>Key material, and never commit these</td>
</tr>
</tbody></table>
<p>The repository&#39;s <code>.rsyncignore</code> reflects this: <code>target</code>, <code>.git</code>, <code>image</code>, <code>nodeA</code>–<code>nodeN</code>,
<code>.env*</code>, <code>.ssh</code>, <code>*.log</code>. When copying the workspace, excluding the heavy generated trees
is what keeps it manageable:</p>
<pre><code class="language-bash">rsync -av --exclude &#39;target&#39; --exclude &#39;jobs&#39; --exclude &#39;blobs&#39; --exclude &#39;artifacts&#39; \\
  source/ destination/
</code></pre>
<h2 id="6-multi-node-checklist">6. Multi-node checklist</h2>
<p>Running several nodes on one machine:</p>
<ul>
<li><input disabled="" type="checkbox"> One directory tree per node, created before launch</li>
<li><input disabled="" type="checkbox"> <code>STORAGE_*</code> and <code>BLOB_STORAGE_DIR</code> exported separately in each shell</li>
<li><input disabled="" type="checkbox"> No two nodes sharing a <code>midcid/</code></li>
<li><input disabled="" type="checkbox"> Distinct <code>listen_addr</code> ports AND Distinct <code>secret_seed</code> per node (identical seeds equal identical peer IDs) in the cluster TOML <a href="https://github.com/omystik/omystik/blob/main/meshes_config/mesh_a_cluster_local.toml">mesh_a_cluster_local</a></li>
</ul>
<hr>
<p><em>Build and run instructions: <a href="/docs/getting-started/">GETTING_STARTED.md</a> · Storage model:
<a href="/docs/architecture/#4-how-storage-works">ARCHITECTURE.md §4</a></em></p>
`}],d=o(),f=window.location.pathname.replace(/\/?$/,`/`),p=u.find(e=>e.route===f),m=u.filter(e=>e.source.startsWith(`docs/`)),h=u.filter(e=>!e.source.startsWith(`docs/`));function g(){let e=e=>(0,d.jsx)(`ul`,{className:`docs-index`,children:e.map(e=>(0,d.jsxs)(`li`,{children:[(0,d.jsx)(`a`,{href:e.route,children:e.title}),(0,d.jsx)(`span`,{children:e.source})]},e.route))});return(0,d.jsxs)(`article`,{className:`docs-content`,children:[(0,d.jsx)(`h1`,{children:`Documentation`}),(0,d.jsx)(`h2`,{children:`Docs`}),e(m),(0,d.jsx)(`h2`,{children:`Project`}),e(h)]})}function _(){return(0,d.jsxs)(`article`,{className:`docs-content`,children:[(0,d.jsx)(`h1`,{children:`Page not found`}),(0,d.jsxs)(`p`,{children:[`Go back to the `,(0,d.jsx)(`a`,{href:`/`,children:`home page`}),` or browse the`,` `,(0,d.jsx)(`a`,{href:a,children:`documentation`}),`.`]})]})}function v(){let t=(0,c.useRef)(null);(0,c.useEffect)(()=>{p&&(document.title=`${p.title} · ØMYSTIK`);let e=decodeURIComponent(window.location.hash.slice(1));e&&document.getElementById(e)?.scrollIntoView({behavior:`instant`})},[]),(0,c.useEffect)(()=>{let e=t.current?.querySelectorAll(`pre > code.language-mermaid`);if(!e?.length)return;let n=!1;return s(async()=>{let{default:e}=await import(`./mermaid.core-CRqxw8r8.js`);return{default:e}},__vite__mapDeps([0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20])).then(({default:t})=>{if(n)return;t.initialize({startOnLoad:!1,theme:`dark`,securityLevel:`strict`});let r=[...e].map(e=>{let t=document.createElement(`div`);return t.className=`mermaid`,t.textContent=e.textContent??``,e.parentElement.replaceWith(t),t});return t.run({nodes:r})}),()=>{n=!0}},[]);let r;return r=f===`/docs/`?(0,d.jsx)(g,{}):p?(0,d.jsxs)(d.Fragment,{children:[(0,d.jsx)(`article`,{ref:t,className:`docs-content`,dangerouslySetInnerHTML:{__html:p.html}}),(0,d.jsx)(`p`,{className:`docs-source`,children:(0,d.jsxs)(`a`,{href:`${n}/blob/main/${p.source}`,children:[`View `,p.source,` on GitHub`]})})]}):(0,d.jsx)(_,{}),(0,d.jsxs)(`div`,{className:`docs`,children:[(0,d.jsx)(i,{}),(0,d.jsx)(`main`,{className:`docs-main`,children:r}),(0,d.jsx)(e,{})]})}(0,l.createRoot)(document.getElementById(`root`)).render((0,d.jsx)(c.StrictMode,{children:(0,d.jsx)(v,{})}));