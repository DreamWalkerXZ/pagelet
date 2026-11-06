# Manage pages by stable ID and reject stale mutations

Management APIs address pages by an immutable page ID, while public reads address them by slug. Require the last-read page version for content, slug, password, ownership, and deletion mutations, rejecting stale versions with HTTP 409 instead of silently overwriting another client's work.
