# Bind page management to users and trust instance administrators

Each page has one owner, and administrators can read, modify, and delete all pages, including password-protected pages; access passwords restrict visitors rather than administrators. Pages are readable by anyone with the link unless password-protected, with the same protection applied to rendered content, raw content, and API reads, while authenticated owners and administrators bypass it.

The initial release uses administrator-created users without public registration or a public page directory. It publishes editable and deletable Markdown pages with externally hosted images, without attachments, revision history, expiration, or collaborative editing, to keep the personal and small-team publishing scope small.

Only administrators can transfer page ownership, including transfers requested by the current owner. User accounts can be deleted only when they own no pages; the initial release does not provide account suspension.

Ownership transfers preserve content, slug, and access password. The previous owner immediately loses management permissions and the owner-specific access-password bypass, retaining only the access available to other visitors unless they are an administrator.

A user's role is fixed at creation: administrator or ordinary user, with no role-change operation. Multiple administrators are allowed, but the final administrator cannot be deleted; the requirement to own no pages applies when deleting administrators too.
