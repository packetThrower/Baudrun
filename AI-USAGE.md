# AI usage

Baudrun was developed in close collaboration with
[Claude (Anthropic)](https://www.anthropic.com/claude), primarily
through [Claude Code](https://claude.com/claude-code). This page
records what that collaboration looks like so contributors and users
can judge for themselves.

## Division of labor

**Claude** did most of the hands-on writing:

- Go backend (serial lifecycle, chipset detection, XMODEM/YMODEM,
  settings + profile + theme + skin stores).
- Svelte frontend (xterm integration, sidebar, profile form,
  settings UI, hex view, paste safety).
- CI/CD pipeline (GitHub Actions matrix, fpm packaging, AppImage,
  AUR PKGBUILD).
- Reference documentation under [docs/](docs/) and most in-code
  comments.
- Mechanical refactors — notably the Svelte 3 → 5 runes migration.

**The maintainer** drives the human side:

- Product direction — what ships, what gets cut, what's on the
  roadmap.
- Review of every change before it lands on `main`.
- Testing against real hardware — Cisco / Aruba / Juniper consoles,
  USB-C bootloaders, CH340/CP210x/FTDI adapters. Claude cannot
  plug a cable into a switch.
- Licensing, security policy, contribution policy, and community
  decisions.

## Seeing it in git history

Commits with substantial AI contribution carry a trailer:

```
Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
```

Filter the log to just those:

```
git log --grep='Co-Authored-By: Claude'
```

## On copyright and GPL-3.0

Under current U.S. Copyright Office guidance (2023–2025), purely
AI-generated output is not copyrightable. Human direction, selection,
and curation generally are. This project is licensed under GPL-3.0
as a whole; that license rests on the copyrightable human
contributions (architecture, design, code review, curation of AI
output) rather than on any claim over the AI-generated drafts
themselves.

Practically: forks and derivatives are governed by GPL-3.0. See
[LICENSE](LICENSE).

## Contributing with AI assistance

Using AI to help with a contribution is welcome — most of this
project was built that way. Two expectations:

- **Review your own output** before opening a PR. Maintainer
  bandwidth is finite; don't outsource review to the reviewer.
- **Attribute it** with a similar `Co-Authored-By` trailer so the
  history stays honest. See [CONTRIBUTING.md](CONTRIBUTING.md) (when
  it exists — TBD) for the full contribution flow.

## Responsibility

Regardless of who drafted a given line, the maintainer is
accountable for the code that ships under this name. Bugs,
regressions, and security issues are the maintainer's to fix.