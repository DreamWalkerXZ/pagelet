# Third-party notices

The executable embeds the following unmodified browser dependencies. Their full
license texts are included in `web/vendor/`.

- Marked 17.0.1, MIT: `web/vendor/marked.LICENSE`, https://github.com/markedjs/marked
- DOMPurify 3.4.17, dual Apache-2.0 / MPL-2.0: `web/vendor/dompurify.LICENSE`, https://github.com/cure53/DOMPurify

`package-lock.json` pins development and source dependencies. Runtime never uses
npm, a CDN, or files beside the executable. After updating these dependencies,
copy `node_modules/marked/lib/marked.esm.js` to `web/vendor/marked.js` and
`node_modules/dompurify/dist/purify.es.mjs` to `web/vendor/purify.js`, update their
license files and this notice, and run the browser suite.
