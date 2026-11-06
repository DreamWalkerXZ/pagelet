import { requestJSON } from "/_/assets/api.js";
import { renderMarkdown } from "/_/assets/renderer.js";

const byId = (id) => document.getElementById(id);
const match = location.pathname.match(/^\/_\/app\/pages\/(\d+)\/edit$/);
let record;
let dirty = false;
let resume;
const status = (message) => {
  byId("status").textContent = message;
};
const error = (message) => {
  byId("error").textContent = message;
};
const preview = () => {
  byId("preview").innerHTML = renderMarkdown(byId("markdown").value);
};

function login(afterLogin) {
  resume = afterLogin;
  byId("login").hidden = false;
  byId("editor").inert = true;
  byId("username").focus();
}
function failure(reason, retry) {
  const messages = {
    400: "Check your slug, password and Markdown. Your entered work is unchanged.",
    401: "Your session has ended. Log in below to continue; your entered work is unchanged.",
    403: "You do not have permission to manage this page. Your entered work is unchanged.",
    404: "Page not found. It may have been deleted. Your entered work is unchanged.",
    409: "Conflict: this page changed elsewhere or the slug is already taken. Your entered work is unchanged. Reload the server version before retrying a changed page.",
    413: "The page is too large. Markdown must be at most 1 MiB.",
    429: "Too many attempts. Please wait before trying again.",
  };
  error(
    messages[reason.status] ||
      "Could not reach the server. Your entered work is unchanged. Please try again.",
  );
  if (reason.status === 401) login(retry);
  byId("reload").hidden = !record || reason.status !== 409;
}
function fill(page) {
  record = page;
  byId("slug").value = page.slug;
  byId("markdown").value = page.markdown;
  byId("access-password").value = "";
  byId("password-action").value = "keep";
  byId("password-action").hidden = false;
  document.querySelector('label[for="password-action"]').hidden = false;
  byId("access-password").disabled = true;
  byId("editor-title").textContent = "Edit page";
  byId("save").textContent = "Save changes";
  byId("danger").hidden = false;
  byId("read").hidden = false;
  byId("read").href = `/${page.slug}`;
  byId("editor").hidden = false;
  byId("reload").hidden = true;
  dirty = false;
  preview();
}
async function load() {
  try {
    if (match) fill(await requestJSON(`/pages/${match[1]}`));
    else {
      await requestJSON("/me");
      byId("editor").hidden = false;
      byId("password-action").hidden = true;
      document.querySelector('label[for="password-action"]').hidden = true;
    }
    status("");
  } catch (reason) {
    status("");
    failure(reason, load);
  }
}
byId("login-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  const button = event.submitter;
  button.disabled = true;
  try {
    await requestJSON("/login", {
      method: "POST",
      body: {
        username: byId("username").value,
        password: byId("account-password").value,
      },
    });
    byId("account-password").value = "";
    byId("login").hidden = true;
    byId("editor").inert = false;
    error("");
    if (location.pathname === "/_/app/login") location.replace("/_/app/new");
    else if (resume) await resume();
  } catch (reason) {
    error(
      reason.status === 401
        ? "Incorrect username or account password."
        : "Login failed. Please wait and try again.",
    );
  } finally {
    button.disabled = false;
  }
});
byId("editor-form").addEventListener("input", () => {
  dirty = true;
  preview();
});
byId("password-action").addEventListener("change", () => {
  byId("access-password").disabled =
    byId("password-action").value !== "replace";
});
byId("editor-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  const button = byId("save");
  button.disabled = true;
  byId("editor").inert = true;
  error("");
  try {
    const body = { markdown: byId("markdown").value };
    if (record || byId("slug").value) body.slug = byId("slug").value;
    if (record) body.version = record.version;
    if (!record || byId("password-action").value === "replace")
      body.access_password = byId("access-password").value;
    if (record && byId("password-action").value === "remove")
      body.access_password = "";
    const creating = !record;
    const page = await requestJSON(record ? `/pages/${record.id}` : "/pages", {
      method: record ? "PATCH" : "POST",
      body,
    });
    fill(page);
    history.replaceState(null, "", `/_/app/pages/${page.id}/edit`);
    status(creating ? "Page published." : "Changes saved.");
  } catch (reason) {
    failure(reason, () => {
      status("Logged in. Your work is ready to save.");
    });
  } finally {
    button.disabled = false;
    byId("editor").inert = !byId("login").hidden;
  }
});
byId("reload").addEventListener("click", async () => {
  if (
    !confirm(
      "Discard your entered changes and load the current server version?",
    )
  )
    return;
  try {
    fill(await requestJSON(`/pages/${record.id}`));
    error("");
    status("Server version loaded.");
  } catch (reason) {
    failure(reason, () => {});
  }
});
byId("delete").addEventListener("click", async () => {
  if (!confirm("Permanently delete this page? This cannot be undone.")) return;
  byId("delete").disabled = true;
  error("");
  try {
    await requestJSON(`/pages/${record.id}`, {
      method: "DELETE",
      body: { version: record.version },
    });
    dirty = false;
    byId("editor").hidden = true;
    status("Page permanently deleted. Its slug is now available.");
  } catch (reason) {
    failure(reason, () => {});
  } finally {
    byId("delete").disabled = false;
  }
});
window.addEventListener("beforeunload", (event) => {
  if (dirty) {
    event.preventDefault();
    event.returnValue = "";
  }
});
if (location.pathname === "/_/app/login") {
  status("");
  login();
} else await load();
