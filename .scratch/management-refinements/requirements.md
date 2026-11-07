# Management interface and service logs

## Confirmed requests

- Redirect `/` to `/_/app/pages`. Send unauthenticated visitors from protected management pages to `/_/app/login`, then send successful logins to `/_/app/pages`.
- Keep navigation consistent throughout signed-in management views. Order links as New page, My pages, Account, All pages, and Users. The last two links are administrator-only.
- Put the My pages Search button to the right of its input.
- Use the same table layout for My pages and All pages. Show Slug, Actions, and Last modified in that order. Link each slug to its public page and show only an Edit button in Actions.
- Move ownership transfer from All pages to below Delete page in the page editor. Show the section only to administrators.
- In the page editor, place Save changes, Read page, and Copy link as buttons under Slug and above Markdown, to the left of the access password controls.
- On both `/_/app/new` and `/_/app/pages/{id}/edit`, give Markdown and Live preview equal bounded heights and synchronize scrolling by source block. Editing follows the block containing the cursor. Manual scrolling follows the corresponding block in either direction. On narrow screens, stack the equal-height panes and disable synchronized scrolling.
- Remove “Publishing is immediate. Pages have no draft state.” from both the new and edit views.
- Present API feedback in popups rather than page-top notices.
- Log important service events, including the listening address, the user who logged in, and graceful shutdown on Ctrl+C.

## Confirmed behavior and boundaries

- Redirect an unauthenticated initial visit to a protected management view to the login page. During editing with unsaved input, keep reauthentication in place and preserve the input. Standalone login always lands on My pages. After an account password change, let the user save their new API token before redirecting to login when they close the dialog. Public reading and page access passwords remain independent of account login.
- Use transient popups for ordinary success messages and dismissible popups for errors. Show one-time API tokens in an explicitly dismissible dialog with a copy action. A subsequent refresh error must not obscure or clear the token. Render response data, loading states, empty states, and pagination in their own view regions.
- Transfer ownership by target user ID after confirmation. Submit only the ownership change, refresh the page version used by the editor, and preserve unsaved inputs. Keep the owner filter in All pages.
- Write timestamped, leveled text logs to stderr for the actual listening address, successful login with username, page publication and deletion, ownership transfer, administrator user creation and deletion, Ctrl+C receipt, and graceful shutdown completion. For failed login and rate limiting, log the event type and source only. Exclude passwords, tokens, cookies, Markdown, and request bodies. Do not log every routine read.
