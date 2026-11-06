# Use paginated metadata search for management lists

Page lists use page-number pagination, defaulting to 20 items and capped at 100, with slug search and descending update-time ordering using stable IDs to break ties. Administrators can additionally filter pages by owner; ordinary users remain restricted to their own pages.

User lists are administrator-only, with page-number pagination and username search. Full-text page-content search is outside the initial scope.
