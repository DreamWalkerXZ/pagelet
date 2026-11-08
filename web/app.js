import { requestJSON } from "/_/assets/api.js";
import { attachPreview } from "/_/assets/editor-preview.js";
import { confirmAction, copy, navigation, notify } from "/_/assets/ui.js";

const byId = (id) => document.getElementById(id);
const match = location.pathname.match(/^\/_\/app\/pages\/(\d+)\/edit$/);
let record;
let dirty = false;
let resume;
let user;
let ownerPage = 1;
const status = notify;
const error = (message) => notify(message, true);
const preview = attachPreview(byId("markdown"), byId("preview"));

function login(afterLogin) {
  resume = afterLogin;
  byId("login").hidden = false;
  byId("editor").inert = true;
  byId("username").focus();
}
function failure(reason, retry, notFound = "Page not found. It may have been deleted. Your entered work is unchanged.") {
  if (reason.status === 401 && !dirty) {
    location.replace("/_/app/login");
    return;
  }
  const messages = {
    400: "Check your slug, password and Markdown. Your entered work is unchanged.",
    401: "Your session has ended. Log in below to continue; your entered work is unchanged.",
    403: "You do not have permission to manage this page. Your entered work is unchanged.",
    404: notFound,
    409: {
      slug_taken: "This slug is already in use. Choose another slug. Your entered work is unchanged.",
      stale_version: "Conflict: this page changed elsewhere. Your entered work is unchanged. Reload the server version before retrying.",
    }[reason.code] ?? "Conflict. Your entered work is unchanged.",
    413: "The page is too large. Markdown must be at most 1 MiB.",
    429: "Too many attempts. Please wait before trying again.",
  };
  error(
    messages[reason.status] ||
      "Could not reach the server. Your entered work is unchanged. Please try again.",
  );
  if (reason.status === 401) login(retry);
  byId("reload").hidden = !record || reason.code !== "stale_version";
}
function fill(page) {
  record = page;
  byId("slug").value = page.slug;
  byId("slug-help").textContent = "Visit Page and Copy Link use the saved address.";
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
  byId("owner-search-form").reset();
  clearOwnerResults();
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
  const slug = byId("slug").value;
  const password = byId("access-password").value;
  const passwordAction = byId("password-action").value;
  if ((record || slug) && !/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(slug)) {
    error("Check your slug. Use lowercase letters, digits, or single interior hyphens.");
    byId("slug").focus();
    return;
  }
  if (record && passwordAction === "replace" && !password) {
    error("Enter a new access password, or select Remove Access Password.");
    byId("access-password").focus();
    return;
  }
  const button = byId("save");
  const effects = [];
  if (record && record.slug !== slug) {
    effects.push(`The address will change from /${record.slug} to /${slug}. The old address will not redirect and may be reused by another page.`);
  }
  if (record && passwordAction === "remove") {
    effects.push("Anyone with the link will be able to read the page after you remove its access password.");
  }
  if (effects.length && !await confirmAction("Save Changes", ...effects)) return;
  button.disabled = true;
  byId("editor").inert = true;
  try {
    const body = { markdown: byId("markdown").value };
    if (record || slug) body.slug = slug;
    if (record) body.version = record.version;
    if (!record || passwordAction === "replace")
      body.access_password = password;
    if (record && passwordAction === "remove")
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
  if (!await confirmAction("Reload Server Version",
    "Discard your entered changes and load the current server version?",
  )) return;
  byId("editor").inert = true;
  try {
    fill(await requestJSON(`/pages/${record.id}`));

    status("Server version loaded.");
  } catch (reason) {
    failure(reason);
  } finally {
    byId("editor").inert = !byId("login").hidden;
  }
});
byId("delete").addEventListener("click", async () => {
  if (!await confirmAction("Delete Permanently",
    `Permanently delete /${record.slug}? This cannot be undone.`,
  )) return;
  byId("delete").disabled = true;
  byId("editor").inert = true;

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
    byId("editor").inert = !byId("login").hidden;
  }
});
byId("copy-link").onclick = () => copy(new URL("/" + record.slug, location.origin).href);

function clearOwnerResults() {
  byId("new-owner").replaceChildren(new Option("Select a user", ""));
  byId("owner-results").textContent = "";
  byId("owner-pagination").hidden = true;
}

async function searchOwners(page) {
  const search = byId("owner-search").value;
  const query = new URLSearchParams({ search, page, per_page: 20 });
  clearOwnerResults();
  byId("editor").inert = true;
  try {
    const result = await requestJSON(`/users?${query}`);
    const options = result.users.filter((candidate) => candidate.id !== record.owner_id)
      .map(({ id, username }) => new Option(`${username} (ID ${id})`, id));
    byId("new-owner").append(...options);
    const pages = Math.max(1, Math.ceil(result.total / result.per_page));
    byId("owner-results").textContent = options.length
      ? `Page ${page} of ${pages}`
      : "No eligible users on this page.";
    byId("owner-pagination").hidden = pages <= 1 && page === 1;
    byId("owner-previous").disabled = page <= 1;
    byId("owner-next").disabled = page >= pages;
    ownerPage = page;
  } catch (reason) {
    failure(reason);
  } finally {
    byId("editor").inert = !byId("login").hidden;
  }
}

byId("owner-search").oninput = clearOwnerResults;
byId("owner-search-form").onsubmit = (event) => {
  event.preventDefault();
  return searchOwners(1);
};
byId("owner-previous").onclick = () => searchOwners(ownerPage - 1);
byId("owner-next").onclick = () => searchOwners(ownerPage + 1);
byId("transfer-form").onsubmit = async (event) => {
  event.preventDefault();
  const selected = byId("new-owner").selectedOptions[0];
  const owner = Number(selected?.value);
  if (!Number.isSafeInteger(owner) || owner < 1 || owner === record.owner_id) return;
  if (!await confirmAction("Transfer Ownership",
    `Transfer /${record.slug} to ${selected.textContent}?`,
    "The previous owner will lose permissions granted by ownership. Administrator permissions are retained. Existing visitor access grants will be revoked.",
    "Unsaved edits will remain in the editor.",
  )) return;
  byId("editor").inert = true;
  try {
    record = await requestJSON(`/pages/${record.id}/owner`, {
      method: "POST",
      body: { version: record.version, owner_id: owner },
    });
    byId("owner-search-form").reset();
    clearOwnerResults();
    status("Ownership transferred. Existing visitor access grants were revoked.");
  } catch (reason) {
    failure(reason, undefined, "The page or selected user is no longer available. Your entered work is unchanged.");
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
