# Security Policy

AnvilML is the headless backend engine of the SindriStudio project. We take the security of the
backend, its Rust↔Python IPC boundary, and the data it manages seriously. Thank you for helping keep
it and its users safe.

## Reporting a vulnerability

**Please report security vulnerabilities privately. Do not open a public issue, pull request, or
discussion for a security problem.**

Use either channel:

- **GitHub Security Advisories (preferred):** open a private report at
  <https://github.com/DrywFiltiarn/AnvilML/security/advisories/new>
- **Email:** trinity3dtech@gmail.com

If you need to encrypt your report or arrange a different channel, say so in an initial email and we
will coordinate.

### What to include

To help us triage quickly, please include where possible:

- A description of the vulnerability and its impact.
- The affected component (a specific Rust crate, the Python worker, the IPC layer, or the REST/WS
  API) and the version, tag, or commit SHA.
- Steps to reproduce, a proof of concept, or a minimal test case.
- Your environment (OS, compute backend, relevant configuration) if applicable.
- Any suggested remediation, if you have one.

## Our commitment

AnvilML is in early, pre-release development maintained by Trinity3D Technologies. We handle reports
on a best-effort basis and aim to:

- **Acknowledge** your report within **5 business days**.
- Provide an initial **assessment** (validity, severity, expected next steps) within **10 business
  days**.
- Keep you informed of progress toward a fix and credit you in the advisory if you wish.

These are targets, not contractual guarantees, and may vary with severity and maintainer
availability.

## Disclosure policy

We follow **coordinated disclosure**: please give us a reasonable opportunity to investigate and
ship a fix before any public disclosure. We will agree a disclosure timeline with you and publish a
GitHub Security Advisory once a fix or mitigation is available.

## Supported versions

AnvilML has not yet reached a tagged release. Until a first stable release is published, **only the
latest commit on the default branch (`main`)** receives security fixes. This table will be updated
once versioned releases begin.

| Version | Supported |
| :-- | :-- |
| `main` (latest) | ✅ |
| Pre-release commits / forks | ❌ |

## Scope and threat model

**In scope:** the AnvilML backend in this repository — the Rust crates, the launcher binary
(`anvilml`), the Python inference worker, the Rust↔Python IPC boundary, and the REST/WebSocket API.

**Out of scope:**

- **BloomeryUI** (the frontend) and **SindriStudio** (the one-click launcher) — report issues in
  those to their respective repositories.
- Vulnerabilities in **third-party dependencies** (PyTorch, diffusers, crates, etc.) — please report
  those upstream; we will update affected pins as fixes become available.
- **User-supplied model weights** and the **user-managed Python virtual environment**, which are
  outside AnvilML's control.

**Important — default security posture.** In the current MVP, AnvilML binds to `127.0.0.1` and has
**no authentication or authorization** (see `ANVILML_DESIGN.md` §10 and §25). It is intended to run
locally, supervised by SindriStudio. Exposing the API to an untrusted network or the public internet
is **not a supported configuration**, and reports based solely on that misconfiguration may be
considered out of scope. Any network exposure should be fronted by your own authenticated reverse
proxy.

## Safe harbor

We will not pursue or support legal action against researchers who act in good faith, make a
reasonable effort to avoid privacy violations and service disruption, and report promptly through
the private channels above. Please do not access or modify data that is not yours, and do not test
against systems you do not own.
