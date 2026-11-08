const links = [
  ["/_/app/new", "New Page"],
  ["/_/app/pages", "My Pages"],
  ["/_/app/account", "Account"],
  ["/_/app/admin/pages", "All Pages"],
  ["/_/app/admin/users", "Users"],
];

export function navigation(user) {
  const nav = document.querySelector('nav[aria-label="Management"]');
  nav.replaceChildren(...links.slice(0, user.role === "administrator" ? 5 : 3)
    .map(([href, label]) => Object.assign(document.createElement("a"), { href, textContent: label })));
  nav.hidden = false;
}

export function notify(message, error = false) {
  const popup = document.createElement("div");
  popup.className = "popup";
  const text = document.createElement("span");
  text.setAttribute("role", error ? "alert" : "status");
  text.textContent = message;
  const close = Object.assign(document.createElement("button"), {
    type: "button", textContent: "Close",
  });
  close.onclick = () => popup.remove();
  popup.append(text, close);
  let stack = document.querySelector(".popup-stack");
  if (!stack) {
    stack = document.createElement("div");
    stack.className = "popup-stack";
    document.body.append(stack);
  }
  stack.replaceChildren(popup);
  if (!error) setTimeout(() => popup.remove(), 4500);
}

document.addEventListener("keydown", (event) => {
  if (event.key !== "Escape" || event.defaultPrevented || document.querySelector("dialog[open]")) return;
  const popup = document.querySelector(".popup-stack .popup");
  if (!popup) return;
  event.preventDefault();
  popup.remove();
});

export async function copy(value) {
  try {
    await navigator.clipboard.writeText(value);
    notify("Copied to clipboard.");
  } catch {
    notify("Clipboard unavailable. Copy the text manually.", true);
  }
}

export function confirmAction(action, ...details) {
  const dialog = document.createElement("dialog");
  dialog.setAttribute("aria-label", action);
  dialog.setAttribute("aria-describedby", "confirmation-details");
  const form = document.createElement("form");
  form.method = "dialog";
  const description = document.createElement("div");
  description.id = "confirmation-details";
  for (const detail of details) {
    const paragraph = document.createElement("p");
    paragraph.textContent = detail;
    description.append(paragraph);
  }
  const cancel = Object.assign(document.createElement("button"), {
    textContent: "Cancel", value: "cancel", autofocus: true,
  });
  const confirm = Object.assign(document.createElement("button"), {
    textContent: action, value: "confirm",
  });
  form.append(description, cancel, " ", confirm);
  dialog.append(form);
  document.body.append(dialog);
  return new Promise((resolve) => {
    dialog.addEventListener("close", () => {
      dialog.remove();
      resolve(dialog.returnValue === "confirm");
    }, { once: true });
    dialog.showModal();
  });
}

export function showToken(token, message, afterClose) {
  const dialog = document.createElement("dialog");
  dialog.setAttribute("aria-label", "Save Your API Token");
  const label = document.createElement("label");
  label.textContent = "New API Token";
  const input = Object.assign(document.createElement("input"), { readOnly: true, value: token });
  label.append(input);
  const description = Object.assign(document.createElement("p"), { textContent: message });
  const copyButton = Object.assign(document.createElement("button"), { textContent: "Copy Token" });
  const warning = Object.assign(document.createElement("p"), {
    textContent: "Save this API token before closing. You won’t be able to view it again.",
  });
  const close = Object.assign(document.createElement("button"), { textContent: "Close" });
  copyButton.onclick = async () => {
    try {
      await navigator.clipboard.writeText(input.value);
      copyButton.textContent = "Copied";
    } catch {
      input.select();
      copyButton.textContent = "Copy Token Manually";
    }
  };
  close.onclick = () => dialog.close();
  dialog.addEventListener("cancel", (event) => event.preventDefault());
  const clear = () => { input.value = ""; dialog.remove(); };
  window.addEventListener("pagehide", clear, { once: true });
  dialog.addEventListener("close", () => {
    window.removeEventListener("pagehide", clear);
    clear();
    afterClose?.();
  });
  dialog.append(description, warning, label, copyButton, " ", close);
  document.body.append(dialog);
  dialog.showModal();
  input.select();
}

export function pageRow(rows, page) {
  const row = rows.insertRow();
  const slug = Object.assign(document.createElement("a"), {
    href: "/" + page.slug, textContent: page.slug,
  });
  const edit = Object.assign(document.createElement("a"), {
    href: `/_/app/pages/${page.id}/edit`, textContent: "Edit", className: "button",
  });
  edit.setAttribute("aria-label", `Edit ${page.slug}`);
  row.insertCell().append(slug);
  row.insertCell().append(edit);
  row.insertCell().textContent = new Date(page.updated_at).toLocaleString();
}
