# Issue Tracker: GitHub

Specs and tickets live in GitHub Issues for DreamWalkerXZ/pagelet.
Use the `gh` CLI from this repository.

## Operations

- Verify the target before creating issues: `gh repo view --json nameWithOwner`.
- Create issues with `gh issue create --title "..." --body-file <path>`.
- Read issues with `gh issue view <number> --json number,title,body,labels,comments`.
- List issues with `gh issue list`, using appropriate state and label filters.
- Comment with `gh issue comment <number> --body-file <path>`.
- Update labels with `gh issue edit <number> --add-label "..."` or `--remove-label "..."`.
- Before closing an issue, post an explanation, then run `gh issue close <number>`.

Use temporary UTF-8 files for multiline bodies.

## Relationships

Use native GitHub sub-issues and blocking dependencies when supported.
Otherwise, put `Part of #<parent>` and `Blocked by: #<number>` at the top
of the child issue body, and link children in the parent's task list.

A ticket is ready when every blocking issue is closed.

## Skill conventions

“Publish to the issue tracker” means create a GitHub issue.
“Fetch the relevant ticket” means read the GitHub issue.

For wayfinding, use one map issue with child decision tickets.
Use `wayfinder:map` for maps and `wayfinder:<type>` for child tickets.
Create these labels only when that workflow is used.

## Pull requests as a triage surface

PRs as a request surface: no.
