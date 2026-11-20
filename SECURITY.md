# Security Policy

## Supported versions

Trazo has not had a tagged release yet. Security fixes land on the `main` branch; please make sure you can reproduce an issue on the latest `main` before reporting it.

| Version | Supported |
| --- | --- |
| `main` | Yes |
| Older commits | No |

## Reporting a vulnerability

**Please do not report security problems in public issues, discussions or pull requests.**

Use GitHub's private vulnerability reporting: open the repository's **Security** tab and click **Report a vulnerability**. If that option is unavailable, contact the maintainer privately using the details on their GitHub profile ([@igors93](https://github.com/igors93)), and include:

- what the problem is and what an attacker could do with it;
- the steps or a proof of concept to reproduce it;
- the Trazo commit and operating system you used.

You can expect an acknowledgement within 7 days. We will keep you informed while we work on a fix and credit you in the release notes, unless you prefer to stay anonymous.

## Security model

Trazo is a desktop application. These are the areas where security matters most:

- **The MCP server.** To make connecting AI assistants easy, the local endpoint `http://127.0.0.1:7799/mcp` has **no authentication** and accepts connections from this computer only. Any program running under your account can read and edit the open plan and save or export files.
- **The public tunnel** (*Abrir túnel*). While it is open, anyone who knows the full address, which includes a random token that changes on every opening, can do the same over the internet. Close the tunnel when you are done; it also closes with Trazo.
- **Project files.** Opening a `.json` project from an untrusted source should never execute code or write outside the paths you choose. Reports of crashes or unexpected file access caused by a crafted project file are welcome.

The roadmap tracks further hardening of the AI connection: a read-only mode and an approval mode in which you confirm each change before it is applied.
