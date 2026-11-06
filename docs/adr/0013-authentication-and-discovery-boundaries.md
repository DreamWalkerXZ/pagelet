# Separate account credentials, visitor access, and discovery

Usernames are unique and fixed, with 3–32 lowercase ASCII letters, digits, or single hyphens, excluding leading and trailing hyphens. Account passwords contain 12–128 characters and may include spaces and Unicode without trimming or rewriting; page access passwords contain 1–128 characters, with an empty value meaning no password.

Web sessions expire seven days after login without renewal and support logout of the current session without a device-list feature. API tokens have no automatic expiry and remain valid until reset, account password change or reset, or account deletion.

Ordinary users can list only their own pages; administrators can list all pages and filter by owner, and only administrators can list users. Visitors cannot enumerate pages or users; before page-password verification, disclose only that a page requires a password, withholding content, ownership, and timestamps.

Rate-limit login and page-password verification by both source IP and target account or page, returning HTTP 429 without permanent account lockout. Use process-local counters that reset on restart, and derive the real client IP behind explicitly trusted proxies without trusting forwarded headers from direct clients.

Trust no proxies by default; configure trusted proxy IPs or CIDRs explicitly. Resolve `X-Forwarded-For` starting at the connected peer and walking right-to-left through trusted proxies, choosing the first untrusted address as the client IP. Deployment instructions require the ingress proxy to overwrite or correctly append the header; requests from untrusted peers cannot supply their own effective source IP through forwarding headers.
