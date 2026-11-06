# Use page-scoped visitor access grants

A correct page access password grants visitor access for 24 hours, shared by raw and API reads and independent of account sessions. Changing or removing the access password, or transferring ownership, invalidates all existing visitor access grants for that page.

Access passwords restrict future server reads rather than recalling content already delivered to a visitor. Store only irreversible password verifiers and disable caching for responses containing page content.

Password verification returns a page-scoped credential for scripts to send in a dedicated request header, while browsers receive an HttpOnly cookie. Keep credentials out of URLs; visitor grants authorize reading only and are separate from bearer account tokens and login sessions.
