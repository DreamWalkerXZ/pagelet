# Bound page size and delete without a recycle bin

Accept valid UTF-8 Markdown, including empty content, up to 1 MiB per page, with a larger request-body limit to accommodate JSON encoding. The initial release imposes no per-user page-count or aggregate-storage quota.

Page deletion permanently removes the page's application records, including content, password verifier, and access grants, immediately releasing its slug, with no recycle bin or restore API. User deletion is permanent and immediately invalidates account sessions and the API token; historical backup retention is governed separately. This is application-level deletion, not a secure-erasure guarantee for database files or storage media.
