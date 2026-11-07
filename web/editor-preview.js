import { renderMarkdown } from "./renderer.js";

export function attachPreview(source, preview) {
  const desktop = matchMedia("(min-width: 761px)");
  const mirror = document.createElement("div");
  mirror.className = "source-measure";
  mirror.setAttribute("aria-hidden", "true");
  source.after(mirror);

  const pending = new WeakMap();
  let blocks = [];
  let markers = [];
  let followFrame;

  function scroll(pane, top) {
    const previous = pane.scrollTop;
    pane.scrollTop = top;
    if (pane.scrollTop !== previous) pending.set(pane, pane.scrollTop);
  }

  function anchors() {
    mirror.style.width = source.clientWidth + "px";
    const base = mirror.getBoundingClientRect().top;
    const target = preview.getBoundingClientRect().top;
    return [[0, 0], ...blocks.map((block, i) => [
      markers[i].getBoundingClientRect().top - base,
      block.getBoundingClientRect().top - target + preview.scrollTop,
    ]), [
      Math.max(0, source.scrollHeight - source.clientHeight),
      Math.max(0, preview.scrollHeight - preview.clientHeight),
    ]];
  }

  for (const [from, to, index] of [[source, preview, 0], [preview, source, 1]]) {
    from.addEventListener("scroll", () => {
      const expected = pending.get(from);
      pending.delete(from);
      if (!desktop.matches || followFrame !== undefined || expected === from.scrollTop) return;
      const limit = from.scrollHeight - from.clientHeight;
      const points = anchors().filter((point) => point[index] >= 0 && point[index] <= limit);
      const next = points.findIndex((point) => point[index] > from.scrollTop);
      if (next <= 0 || from.scrollTop >= limit) {
        scroll(to, from.scrollTop ? to.scrollHeight - to.clientHeight : 0);
        return;
      }
      const before = points[next - 1];
      const after = points[next];
      const fraction = (from.scrollTop - before[index]) / (after[index] - before[index]);
      scroll(to, before[1 - index] + fraction * (after[1 - index] - before[1 - index]));
    });
  }

  return function update(followCursor = false) {
    if (followFrame !== undefined) cancelAnimationFrame(followFrame);
    followFrame = undefined;
    preview.innerHTML = renderMarkdown(source.value, true);
    blocks = [...preview.querySelectorAll(":scope > [data-source-offset]")];
    const fragment = document.createDocumentFragment();
    let offset = 0;
    markers = blocks.map((block) => {
      const start = Number(block.dataset.sourceOffset);
      const marker = document.createElement("span");
      fragment.append(source.value.slice(offset, start), marker);
      offset = start;
      return marker;
    });
    fragment.append(source.value.slice(offset), "\n");
    mirror.replaceChildren(fragment);
    if (followCursor && desktop.matches) {
      // Wait for the textarea to scroll its caret into view before aligning the block.
      followFrame = requestAnimationFrame(() => {
        followFrame = undefined;
        if (!desktop.matches) return;
        const i = blocks.findLastIndex((block) => Number(block.dataset.sourceOffset) <= source.selectionStart);
        if (i < 0) {
          scroll(preview, 0);
          return;
        }
        const [sourceTop, previewTop] = anchors()[i + 1];
        const room = Math.max(0, preview.clientHeight - blocks[i].getBoundingClientRect().height);
        const inset = Math.max(0, Math.min(sourceTop - source.scrollTop, room));
        scroll(preview, previewTop - inset);
      });
    }
  };
}
