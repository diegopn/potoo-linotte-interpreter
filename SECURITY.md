# Security Policy

## Supported versions

The latest published release and the `main` branch are supported for security reports. Reports affecting `develop` are also considered while changes are in progress; older releases may no longer receive security updates.

## Report a vulnerability

Do not publish vulnerability details in issues, pull requests, discussions, or public comments. If GitHub provides the private **Report a vulnerability** form on the repository’s Security page, use it. Otherwise, use the [Potoo contact page](https://diegopn.github.io/#contato) only to request a private reporting channel. Do not include technical details or a proof of concept in that first message.

After arranging a private channel, include the affected version or commit, Rust version, operating system, reproduction steps, impact, and a minimal `Linotte` example. Remove credentials, personal information, and unrelated data.

For non-security issues, see [CONTRIBUTING.md](CONTRIBUTING.md). For conduct concerns, see [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## Security boundaries

Linotte books execute in the embedding process. The `Host` interface controls access to external services, but it is not an operating-system sandbox and does not by itself impose general CPU or memory quotas. Applications that run untrusted books should use operating-system process isolation and apply their own resource limits.
