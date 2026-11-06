# Ship one executable with local persistent storage

Pagelet targets lightweight self-hosting: distribute one Rust executable with embedded frontend assets and use a local SQLite database without an external database service. Runtime database and configuration files are allowed; backing up the executable alone does not back up instance data.
