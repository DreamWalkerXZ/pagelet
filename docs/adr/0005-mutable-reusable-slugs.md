# Allow slug changes and immediate reuse without redirects

Page slugs are mutable, and deleting a page or changing its slug immediately releases the previous slug for reuse without a redirect or reservation. Existing links can therefore return 404 or point to a different page after reuse; changing a slug preserves the page's content, owner, and access password.

Custom slugs contain 1–64 lowercase ASCII letters, digits, or hyphens, with no leading, trailing, or consecutive hyphens. Invalid or occupied custom slugs are rejected without normalization; generated slugs use 12 lowercase ASCII letters and digits and are regenerated on collision.
