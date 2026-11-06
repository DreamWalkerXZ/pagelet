import { Marked } from "./vendor/marked.js";
import DOMPurify from "./vendor/purify.js";

const markdown = new Marked({
  gfm: true,
  breaks: false,
  renderer: { html: () => "" },
});
DOMPurify.addHook("afterSanitizeAttributes", (node) => {
  for (const attribute of ["href", "src"]) {
    if (!node.hasAttribute(attribute)) continue;
    const url = node.getAttribute(attribute);
    try {
      const protocol = new URL(url, location.href).protocol;
      const allowed =
        attribute === "href"
          ? ["http:", "https:", "mailto:"]
          : ["http:", "https:"];
      if (!allowed.includes(protocol)) node.removeAttribute(attribute);
    } catch {
      node.removeAttribute(attribute);
    }
  }
  if (node.tagName === "INPUT") {
    node.setAttribute("type", "checkbox");
    node.setAttribute("disabled", "");
  }
});

export function renderMarkdown(source) {
  return DOMPurify.sanitize(markdown.parse(source), {
    ALLOWED_TAGS: [
      "a",
      "blockquote",
      "br",
      "code",
      "del",
      "em",
      "h1",
      "h2",
      "h3",
      "h4",
      "h5",
      "h6",
      "hr",
      "img",
      "input",
      "li",
      "ol",
      "p",
      "pre",
      "strong",
      "table",
      "tbody",
      "td",
      "th",
      "thead",
      "tr",
      "ul",
    ],
    ALLOWED_ATTR: [
      "href",
      "src",
      "alt",
      "title",
      "start",
      "align",
      "type",
      "checked",
      "disabled",
    ],
  });
}
