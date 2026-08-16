# Support

## Where to ask for help

- Use GitHub Issues for reproducible bugs and feature requests.
- Use GitHub Discussions, if enabled, for setup questions and general usage.
- Use GitHub private security reporting for vulnerabilities; see SECURITY.md.

## What to include in a bug report

1. Application version and operating system version.
2. CPU architecture, such as Apple Silicon, Intel, Linux x86_64, or Windows
   x86_64.
3. The release artifact type, such as DMG, AppImage, or NSIS installer.
4. Sanitized application logs and the visible error message.
5. The operating mode: `frp-panel` Client, native `frpc`, or local `frps`.
6. For local `frps`, whether the server process is running, whether
   `webServer.port` is enabled, and whether the Dashboard is shown as
   available or unavailable.
7. Whether the relevant endpoint uses a publicly trusted certificate or a
   self-signed certificate.

Never include a Client Secret, Dashboard password, full production URL, cookie,
token, or private client/server configuration in a public issue.
