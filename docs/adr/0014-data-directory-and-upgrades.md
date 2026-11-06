# Keep persistent data together and migrate forward

Store persistent instance data in one configurable data directory. Automatically apply database migrations on startup, but refuse to start against a database created by a newer schema version; database downgrades are unsupported.

The initial backup procedure is to stop the service and copy the entire data directory, retaining a backup before upgrades. An online backup API is outside the initial scope.
