# Pagelet

Pagelet is a self-hosted Markdown publishing service for individuals and small teams.

## Language

**Page**:
A published Markdown document belonging to one owner, readable through its link and optionally protected by an access password.
_Avoid_: Paste, note

**Owner**:
The user to whom a page belongs and who can manage it; only an administrator can transfer ownership.
_Avoid_: Author, editor

**Administrator**:
A trusted instance user who manages users and can read, modify, and delete every page, including password-protected pages.
_Avoid_: Superuser

**Access password**:
A page-specific secret that restricts visitor access to its content, including raw content and API reads; the owner and administrators can bypass it when authenticated.
_Avoid_: Edit token, account password

**Raw content**:
The Markdown source of a page, without rendered formatting.
_Avoid_: Rendered page

**Slug**:
The mutable, instance-wide unique URL segment identifying a page, using lowercase letters, digits, and single hyphens with no leading or trailing hyphen. A released slug can be reused.
_Avoid_: Page ID

**API token**:
The single resettable credential issued when a user is created, granting exactly that user's permissions; the user or an administrator can reset it.
_Avoid_: Edit token

**Page access grant**:
A page-specific visitor authorization valid for 24 hours after password verification, revoked when the page's password changes or is removed, or ownership is transferred.
_Avoid_: Login session, API token

**Page ID**:
An immutable identifier for a page, retained across slug changes and ownership transfers and used for management operations.
_Avoid_: Slug

**Page version**:
The revision used to detect conflicting page mutations; clients must supply the version they last read when changing or deleting a page.
_Avoid_: Revision history
