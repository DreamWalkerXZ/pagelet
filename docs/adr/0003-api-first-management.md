# Implement the API before the web management interface

The web interface wraps the API rather than introducing separate management behavior, so implement the API before designing web pages. Each user receives one API token at account creation with exactly the user's permissions, resettable by that user or an administrator; token collections and configurable token scopes are outside this model.

The first delivery includes the API, OpenAPI documentation, and initialization and recovery CLI commands. Public reading pages, editor preview, and the management interface follow in a second phase.

Full API tokens are returned only when created or reset and cannot subsequently be retrieved. A reset immediately invalidates the previous token; account password changes and resets rotate the token and invalidate all web sessions.

Scripts authenticate with bearer API tokens. The web interface uses separate HttpOnly cookie sessions rather than storing API tokens in browser storage; cookie-authenticated mutations require CSRF protection. Resetting an API token alone does not invalidate web sessions.
