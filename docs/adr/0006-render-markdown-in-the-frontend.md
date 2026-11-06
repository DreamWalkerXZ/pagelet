# Render Markdown in the frontend

The API returns Markdown source and page metadata without an HTML rendering endpoint. The frontend shares GFM rendering between public reading and editor preview, accepting JavaScript-dependent reading and deferring dedicated support for search indexing, link previews, and reading without JavaScript.

Disable embedded raw HTML, filter dangerous URL schemes, and sanitize rendered content to prevent published content from executing scripts or accessing credentials. The server authorizes source access before returning Markdown, including password-protected content.

The initial renderer supports standard Markdown and GFM tables, task lists, strikethrough, and autolinks. Task lists are display-only and code blocks have no syntax highlighting; mathematical notation, Mermaid, footnotes, and GitHub-specific user and issue references are outside the initial scope.
