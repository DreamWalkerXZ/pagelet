# Keep persistent data together and migrate forward

Store persistent instance data in one configurable data directory. Automatically apply database migrations on startup, but refuse to start against a database created by a newer schema version; database downgrades are unsupported.

Preserve existing user IDs during upgrades. From schema version 5 onward, retain the highest assigned user ID across deletions so an administrator action based on a stale user list cannot target a replacement account. IDs deleted under earlier schemas cannot be recovered from the database.

The initial backup procedure is to stop the service and copy the entire data directory, retaining a backup before upgrades. An online backup API is outside the initial scope.
