# Security

Verity reads untrusted files: saves, Wonder Cards and, on the DS, the user's ROM. A file that
crashes the editor, makes it write a corrupt save, or runs code is a security issue.

## Reporting

Report it privately through GitHub's
[private vulnerability reporting](https://github.com/llamington/verity/security/advisories/new),
not in a public issue. Include the file that triggers it if you can. You'll get a reply within a
week.

## Scope

- The web app runs entirely in the browser. Saves never leave the device, so any way for save data to
  leave it is in scope too.
- Only the latest release and the live site are supported.
