## Problem Statement

Individuals and small teams need a lightweight, open-source way to publish and maintain Markdown pages under short, shareable URLs on their own infrastructure. They need account-bound editing, optional password-protected reading, and an automation API without external database or email services, anonymous edit tokens, or a complex deployment stack.

## Solution

Build Pagelet as a Rust service distributed as one executable, with local SQLite persistence and eventually embedded frontend assets. Administrators provision users, authenticated users publish pages, and visitors read pages by slug, subject to optional access passwords. Deliver a complete API, raw access, OpenAPI specification, and local administration CLI first; then deliver a browser interface that wraps the same API and renders Markdown client-side.

## User Stories

1. As an operator, I want one executable with no external database requirement, so that deployment stays simple.
2. As an operator, I want persistent data in one configurable directory, so that I can manage and back up the instance.
3. As an operator, I want local interactive initialization with hidden password input, so that I can create the first administrator without public setup exposure.
4. As an operator, I want an uninitialized instance to refuse to serve, so that no default credentials or setup race exists.
5. As an administrator, I want local CLI password recovery, so that I can regain access without email integration.
6. As an operator, I want configurable HTTP listening with a loopback default, so that I can deploy behind my HTTPS reverse proxy.
7. As an operator, I want explicitly trusted proxies and real source-IP resolution, so that forwarding headers cannot bypass abuse controls.
8. As an operator, I want forward database migrations and rejection of newer schemas, so that incompatible binaries do not silently damage data.
9. As an operator, I want a documented stopped-service backup procedure, so that I can preserve data before upgrades.
10. As an administrator, I want to create ordinary users and other administrators, so that I control who can publish.
11. As an administrator, I want usernames and roles to remain fixed, so that account identity and authority are predictable.
12. As an administrator, I want the final administrator to be undeletable, so that the instance retains an administrator.
13. As an administrator, I want to delete only users who own no pages, so that account deletion cannot orphan or silently destroy content.
14. As an administrator, I want to list and search users, so that I can manage accounts.
15. As a user, I want username-and-password login, so that I can manage pages in a browser.
16. As a user, I want to change my password, so that I can maintain account security.
17. As an administrator, I want to reset user passwords, so that I can restore access.
18. As a user, I want password changes to revoke all sessions and rotate my API token, so that old account credentials stop working.
19. As a user, I want a single API token issued with my account, so that scripts can act with my permissions.
20. As a user, I want to reset my API token, so that I can replace an exposed credential.
21. As an administrator, I want to reset another user's API token, so that I can help recover or secure access.
22. As a user, I want tokens revealed only at issuance or reset, so that saved account data cannot expose existing token values.
23. As a browser user, I want a separate HttpOnly login session, so that the interface need not store my API token in browser storage.
24. As a user, I want logout and a fixed seven-day session lifetime, so that browser access has a clear end.
25. As a user, I want to publish Markdown with a generated or custom slug, so that I can share a short URL.
26. As a user, I want invalid or occupied custom slugs rejected, so that the service does not silently change my requested address.
27. As an owner, I want to edit my page's Markdown and slug, so that I can maintain its content and address.
28. As an owner, I want empty pages and immediate publication, so that I can create a page before writing its content.
29. As an owner, I want creation and update timestamps, so that I can identify recently changed pages.
30. As an owner, I want a stable page ID across slug changes, so that management operations keep targeting the same page.
31. As an owner, I want stale changes and deletions rejected, so that concurrent clients cannot silently overwrite work.
32. As an owner, I want to list and search my pages with pagination, so that I can find content without seeing other users' private management data.
33. As an administrator, I want to list all pages and filter by owner, so that I can manage instance content.
34. As an administrator, I want to read, edit, and delete any page regardless of its access password, so that I can administer the instance.
35. As an administrator, I want to transfer page ownership, so that pages can be reassigned before account deletion.
36. As a new owner, I want the previous owner to lose owner privileges immediately, so that management follows current ownership.
37. As an owner, I want permanent page deletion and immediate slug reuse, so that I can remove content and release its address.
38. As a visitor, I want to read a page through its known slug, so that I can follow a shared link without an account.
39. As a visitor, I want raw Markdown access, so that I can download or reuse the source.
40. As an owner, I want an optional access password covering every content-read path, so that visitors cannot bypass protection through raw access or the API.
41. As a visitor, I want password verification to grant 24 hours of page-specific reading, so that I do not resend the password on every request.
42. As a script user, I want to send a page access grant in a dedicated header, so that I can read protected pages without putting secrets in URLs.
43. As a browser visitor, I want an HttpOnly page access cookie, so that protected-page reading works without exposing the grant to browser scripts.
44. As an owner, I want password changes, password removal, and ownership transfers to revoke previous page access grants, so that access reflects the current policy.
45. As an owner, I want protected-page metadata withheld before authorization, so that a password challenge does not disclose ownership or timestamps.
46. As a visitor, I want correct GFM reading and editor preview in the browser, so that tables, task lists, strikethrough, and autolinks display consistently.
47. As a browser user, I want embedded HTML and dangerous links prevented from executing code, so that published content cannot steal credentials.
48. As an API consumer, I want a public OpenAPI contract, so that I can discover and automate supported operations.
49. As an operator, I want login and page-password attempts rate-limited by source and target, so that brute-force attempts are constrained without permanently locking accounts.
50. As a user, I want oversized or invalid content rejected before persistence, so that the instance enforces a predictable page-size limit.

## Implementation Decisions

### Delivery and architecture

- Phase one delivers the Rust API, raw-content endpoint, public OpenAPI specification, initialization CLI, and administrator recovery CLI. Phase two delivers public reading, editor preview, and management interfaces with embedded frontend assets.
- SQLite is local persistent storage; runtime database and configuration files are allowed despite single-executable distribution.
- The web interface wraps the API and has no separate authorization or management rules. The API returns Markdown and metadata, not rendered HTML.
- Separate account authentication, page authorization, persistence, and transport responsibilities only as needed; no existing application modules or test infrastructure exist.

### Accounts and credentials

- Administrators create all users; public registration and email integration are absent. Each user has a fixed, unique username and an immutable administrator or ordinary-user role.
- Usernames contain 3–32 lowercase ASCII letters, digits, or hyphens, excluding leading, trailing, and consecutive hyphens.
- Account passwords contain 12–128 characters, allowing spaces and Unicode without trimming or normalization. Page access passwords contain 1–128 characters; an empty value means no password. Store irreversible password verifiers.
- Users change their own passwords; administrators reset user passwords; the local CLI recovers administrator access. Password changes and resets rotate the account API token and invalidate all account sessions.
- Exactly one API token exists per user, issued at creation and inheriting all current account permissions. The user or an administrator can reset it. Reveal the full token only on creation or reset; it is not subsequently retrievable. Rotation immediately invalidates the previous token. Tokens do not expire automatically.
- Scripts use bearer account tokens. Browsers use separate HttpOnly session cookies with a fixed seven-day lifetime and current-session logout. Token reset alone does not invalidate browser sessions; cookie-authenticated mutations require CSRF protection.
- Delete a user only when they own no pages, and never delete the final administrator. Deletion invalidates account sessions and the API token. Provide no role-change or account-suspension operation.

### Pages, ownership, and concurrency

- A page has an immutable page ID, mutable unique slug, Markdown source, optional access password verifier, owner, creation and update timestamps, and page version. No separate title or draft state exists; creation publishes immediately, including empty content.
- Only authenticated users can publish. Owners manage their pages; administrators can read, edit, and delete all pages. Only administrators can transfer ownership, including when the current owner requests a transfer.
- Transfers preserve page content, slug, and access password. The previous owner immediately loses management and password-bypass privileges unless independently authorized as an administrator, retaining ordinary visitor access where applicable.
- Management operations address stable page IDs. Public reads address slugs. Require the last-read page version for content, slug, password, ownership, and deletion mutations; reject stale versions with HTTP 409. Successful mutations advance the version.
- Custom slugs contain 1–64 lowercase ASCII letters, digits, or single hyphens, excluding leading and trailing hyphens. Reject invalid or occupied values without rewriting them. Generate random slugs with 12 lowercase ASCII letters and digits, retrying collisions.
- Slug changes release the old slug immediately without redirects or reservations and preserve content, owner, and access password. Reused slugs may make an old link point to another page.
- Accept valid UTF-8 Markdown up to 1 MiB measured in UTF-8 bytes. Permit empty content. Set a larger bounded request-body limit that accommodates JSON encoding overhead. No per-user page-count or aggregate-storage quota is included.
- Permanently delete page application records, password verifiers, and access grants; release the slug immediately. No recycle bin or restore endpoint exists. This is not a secure-erasure guarantee for storage media or historical backups.

### Reading and discovery

- Known-link reading is public by default, with no public page directory. An access password protects Markdown content across API and raw access; authenticated owners and administrators bypass it.
- Before visitor password verification, disclose only that a password is required, without content, owner information, or timestamps. Visitors cannot enumerate pages or users.
- Correct password verification issues a page-scoped read-only grant lasting 24 hours. Scripts receive a credential to submit in a dedicated header; browsers receive an HttpOnly cookie. Never place credentials in URLs.
- Changing or removing an access password or transferring ownership invalidates every existing grant for that page. Grants are bound to page identity so slug reuse cannot authorize a different page.
- Disallow caching of responses containing page content. Access changes restrict future server reads and cannot recall content already downloaded or displayed.
- Ordinary users list only their own pages. Administrators list all pages and may filter by owner; only administrators list users.
- Page lists support slug search and descending update-time ordering with stable ID tie-breaking. User lists support username search. Use page-number pagination with 20 items by default and a 100-item maximum.

### HTTP surface and rendering

- Public reading uses the root slug route. Raw content, API version 1, the management interface, the OpenAPI specification, and assets use the agreed reserved underscore namespace.
- Publish the OpenAPI specification publicly, describing request and response schemas, authentication methods, password-grant usage, validation errors, conflict responses, and rate-limit responses. The specification contains no instance secrets.
- Implement read, create, edit, delete, and list operations for pages; administrator-only ownership transfer, user creation, deletion and listing; account login, logout, password change and reset, token reset, and page-password verification. No role-change operation is exposed.
- Browser rendering supports standard Markdown and GFM tables, display-only task lists, strikethrough, and autolinks. Share rendering behavior between preview and reading.
- Disable raw embedded HTML, filter dangerous URL schemes, and sanitize rendered content. Externally hosted images use Markdown image syntax; code blocks initially have no syntax highlighting.
- Reading requires JavaScript; dedicated support for search indexing, social link previews, and no-JavaScript reading is deferred.

### Operation and abuse controls

- Initialize through the local CLI with hidden password input and one-time display of the first administrator's token. Refuse to serve an uninitialized instance and provide no default credentials or public setup endpoint.
- Default HTTP listening is loopback port 8080, configurable through CLI arguments or environment variables. Public HTTPS terminates at a reverse proxy, with explicit external-URL and trusted-proxy configuration.
- Trust no proxies by default. Configure trusted proxy IPs or CIDRs. Resolve the real source IP by walking the forwarded-for chain right-to-left from the connected peer, skipping trusted proxies and selecting the first untrusted address. Ignore forwarding headers from untrusted peers. Document that ingress proxies must overwrite or correctly append the header.
- Rate-limit login and page-password verification by both effective source IP and target account or page. Return HTTP 429 without permanent account lockout. Initial counters are process-local and reset on restart.
- Keep persistent data in a configurable directory. Apply forward migrations automatically and refuse to open databases with newer schema versions. Document stopping the service and backing up the entire directory before upgrades; no database downgrade or online backup interface is included.

## Testing Decisions

The following testing seams were confirmed during specification review.

- Test external behavior and observable persisted outcomes, rather than helper functions, SQL shapes, module layout, or private representations. No prior application tests or existing test seams exist in this repository.
- Use the HTTP application boundary as the primary integration seam, backed by a real temporary SQLite database and real authentication, authorization, and persistence behavior. Avoid mocking the database or duplicating production rules in test fixtures.
- Cover the account-to-page lifecycle: administrator initialization, user creation, password login and bearer authentication, publication, authorized reading, editing, slug changes, ownership transfer, deletion, and restart persistence.
- Exercise the authorization matrix for visitors, owners, other ordinary users, and administrators across reading, raw access, lists, mutations, user management, and ownership transfer. Verify protected-page metadata is withheld and grants cannot mutate pages or survive forbidden transitions or slug reuse.
- Check credential issuance and one-time disclosure, immediate token revocation, password-triggered token rotation and session revocation, logout, fixed session and grant expiry, deletion revocation, and CSRF rejection for cookie-authenticated mutations. Use controllable time where necessary rather than waiting in real time.
- Verify validation boundaries: accepted and rejected slugs and usernames, uniqueness, generated-slug collision handling, password character boundaries, Unicode byte-size limits, invalid UTF-8, empty pages, encoded request sizes, and page-list pagination and visibility.
- Exercise concurrent operations with observable outcomes: stale page versions return 409, competing mutations do not silently overwrite, concurrent deletion cannot remove the final administrator, and user deletion cannot race with page creation or ownership assignment to orphan pages.
- Test rate limits and trusted-proxy behavior with direct clients, forged headers, trusted single and multiple proxy chains, and target-based attempts across multiple source IPs. Verify HTTP 429 without permanent lockout.
- Validate public OpenAPI against the actual HTTP behavior, including security requirements and documented error responses.
- Add a small executable CLI smoke suite for initialization, uninitialized startup refusal, administrator recovery, restart persistence, migration compatibility, and configurable data directories. CLI process behavior is a necessary boundary separate from HTTP.
- In phase two, add a small real-browser suite for login, editing and preview, protected-page reading, raw access, logout, and token display. Include GFM fixtures and malicious HTML/URL fixtures, checking rendered behavior and credential isolation rather than a specific renderer's internals.
- No performance targets or hardware benchmark requirements were agreed. Do not invent load-related acceptance thresholds.

## Out of Scope

- Public registration, anonymous publication, edit tokens, email delivery or recovery, federated login, account suspension, mutable usernames, and role changes.
- Multiple API tokens per user, token scopes, token inventories, automatic token expiry, device management, and configurable credential-lifetime features.
- Collaborative editing, owner-initiated ownership transfer, attachments, uploaded images, revision history, drafts, separate title fields, page expiration, recycle bins, and restoration APIs.
- Public page or user directories, content full-text search, per-user storage quotas, and public-discovery features.
- Backend HTML rendering, raw embedded HTML, syntax highlighting, mathematical notation, Mermaid, footnotes, and GitHub-specific user or issue linking.
- Dedicated search-engine indexing, social previews, and reading without JavaScript.
- External database services, horizontal multi-instance coordination, distributed rate limiting, built-in TLS certificate management, online backups, database downgrades, and secure erasure of storage media or backups.
- Frontend delivery in phase one; business implementation or automatic ticket decomposition as part of writing this specification.

## Further Notes

- This specification synthesizes the confirmed design interview and accepted repository decisions. No prototype or business implementation exists yet.
- This is the full product specification with two delivery phases; completion of phase one alone does not complete the full specification.
- Keep exact endpoint operation names, schema field names, header names, frontend tooling, Rust libraries, and internal module organization as implementation details consistent with these contracts. Select abuse-control thresholds and malformed-proxy-header handling conservatively and document them during implementation, without adding new product features.
- The route allocation is fixed: public reading at `/<slug>`, raw Markdown at `/_/raw/<slug>`, API at `/_/api/v1/*`, web management at `/_/app/*`, public OpenAPI at `/_/openapi.json`, and assets at `/_/assets/*`.
- Administrator access is deliberately trusted, including password-protected pages. Access passwords are not end-to-end encryption and do not protect content from the instance operator.
- Mutable reusable slugs deliberately trade permanent link identity for flexibility; stable IDs remain the management identity.
- The testing seams were confirmed before publication. This specification is ready for task decomposition without additional triage.
