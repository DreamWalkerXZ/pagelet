import { requestJSON } from "/_/assets/api.js";
import { attachPreview } from "/_/assets/editor-preview.js";
import { copy, navigation, notify } from "/_/assets/ui.js";

const byId = (id) => document.getElementById(id);
const match = location.pathname.match(/^\/_\/app\/pages\/(\d+)\/edit$/);
let record;
let dirty = false;
let resume;
let user;
const status = notify;
const error = (message) => notify(message, true);
const preview = attachPreview(byId("markdown"), byId("preview"));

function login(afterLogin) {
  resume = afterLogin;
  byId("login").hidden = false;
  byId("editor").inert = true;
  byId("username").focus();
}
function failure(reason, retry) {
  if (reason.status === 401 && !dirty) {
    location.replace("/_/app/login");
    return;
  }
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
  byId("editor-title").textContent = "Edit Page";
  byId("save").textContent = "Save Changes";
  byId("danger").hidden = false;
  byId("read").hidden = false;
  byId("copy-link").hidden = false;
  byId("transfer").hidden = user.role !== "administrator";
  byId("read").href = `/${page.slug}`;
  byId("editor").hidden = false;
  byId("reload").hidden = true;
  dirty = false;
  preview();
}
async function load() {
  try {
    user = await requestJSON("/me");
    navigation(user);
    if (match) fill(await requestJSON(`/pages/${match[1]}`));
    else {
      byId("editor").hidden = false;
      byId("password-action").hidden = true;
      document.querySelector('label[for="password-action"]').hidden = true;
    }
    byId("status").hidden = true;
  } catch (reason) {
    byId("status").hidden = true;
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
    if (location.pathname === "/_/app/login") location.replace("/_/app/pages");
    else {
      user = await requestJSON("/me");
      byId("login").hidden = true;
      byId("editor").inert = false;
      navigation(user);
      byId("transfer").hidden = !record || user.role !== "administrator";
      if (resume) await resume();
      else status("Logged in.");
    }
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
byId("editor-form").addEventListener("input", (event) => {
  dirty = true;
  if (event.target.id === "markdown") preview(true);
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

    status("Server version loaded.");
  } catch (reason) {
    failure(reason);
  }
});
byId("delete").addEventListener("click", async () => {
  if (!confirm("Permanently delete this page? This cannot be undone.")) return;
  byId("delete").disabled = true;

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
byId("copy-link").onclick = () => copy(new URL("/" + record.slug, location.origin).href);
byId("transfer-form").onsubmit = async (event) => {
  event.preventDefault();
  const owner = Number(byId("new-owner").value);
  if (!Number.isSafeInteger(owner) || owner < 1) return;
  if (!confirm("Transfer ownership to user " + owner + "? Unsaved edits will remain in the editor.")) return;
  byId("editor").inert = true;
  try {
    record = await requestJSON(`/pages/${record.id}/owner`, {
      method: "POST", body: { version: record.version, owner_id: owner },
    });
    byId("transfer-form").reset();
    status("Ownership transferred. Previous owner privileges and visitor access grants were revoked.");
  } catch (reason) {
    failure(reason);
  } finally {
    byId("editor").inert = !byId("login").hidden;
  }
};
window.addEventListener("beforeunload", (event) => {
  if (dirty) {
    event.preventDefault();
    event.returnValue = "";
  }
});
if (location.pathname === "/_/app/login") {
  byId("status").hidden = true;
  login();
} else await load();
