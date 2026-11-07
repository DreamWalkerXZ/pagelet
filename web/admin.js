import { requestJSON } from "/_/assets/api.js";
import { navigation, notify, pageRow, showToken } from "/_/assets/ui.js";
const $ = (id) => document.getElementById(id);
const users = location.pathname.endsWith("/users");
let page = 1;
let administrator;
function failure(reason, deletion = false) {
  const messages = {
    400: "Check the entered values. Passwords require 12–128 characters and IDs must identify existing users.",
    401: "Your session has ended. Log in again to continue.",
    403: "Administrator access is required. You do not have permission for this operation.",
    404: "The page or user no longer exists. Refresh the list.",
    409: deletion
      ? "User cannot be deleted. Transfer or delete all owned pages first; the final administrator cannot be deleted."
      : "Conflict: the page changed elsewhere or the username is already taken. Refresh the list before retrying.",
  };
  notify(messages[reason.status] || "The operation failed. Please try again.", true);
  if (reason.status === 401) location.replace("/_/app/login");
}
function cell(row, value) {
  const td = row.insertCell();
  td.textContent = value;
  return td;
}
function button(parent, name, action) {
  const b = document.createElement("button");
  b.type = "button";
  b.textContent = name;
  b.addEventListener("click", async () => {

    b.disabled = true;
    try {
      await action();
    } catch (reason) {
      failure(reason, name === "Delete user");
    } finally {
      b.disabled = false;
    }
  });
  parent.append(b);
}
async function load() {
  $("rows").replaceChildren();
  const query = new URLSearchParams({
    page,
    per_page: 20,
    search: $("search").value,
  });
  if (!users && $("owner").value) query.set("owner_id", $("owner").value);
  const result = await requestJSON(`/${users ? "users" : "pages"}?${query}`);
  $("rows").replaceChildren();
  for (const record of result[users ? "users" : "pages"]) {
    if (!users) {
      pageRow($("rows"), record);
      continue;
    }
    const row = $("rows").insertRow();
    cell(row, record.id);
    cell(row, record.username);
    cell(row, record.role);
    const actions = row.insertCell();
    const label = document.createElement("label");
    label.textContent = "New account password";
    const password = document.createElement("input");
    password.type = "password";
    password.autocomplete = "new-password";
    label.append(password);
    actions.append(label);
    button(actions, "Reset password", async () => {
      if (
        !confirm(
          "Reset this password? All account sessions and the previous API token will be revoked.",
        )
      )
        return;
      const result = await requestJSON(`/users/${record.id}/password`, {
        method: "POST",
        body: { password: password.value },
      });
      password.value = "";
      showToken(
        result.token,
        "Password reset. All account sessions were revoked.",
        record.id === administrator.id ? () => location.replace("/_/app/login") : undefined,
      );
    });
    button(actions, "Reset API token", async () => {
      const result = await requestJSON(`/users/${record.id}/token`, { method: "POST" });
      showToken(result.token, "API token reset. The previous token is invalid.");
    });
    button(actions, "Delete user", async () => {
      if (
        !confirm(
          `Permanently delete ${record.username}? This cannot be undone.`,
        )
      )
        return;
      await requestJSON(`/users/${record.id}`, { method: "DELETE" });
      await load();
      notify("User permanently deleted.");
    });

  }
  $("count").textContent = `${result.total} ${users ? "users" : "pages"}`;
  $("page-number").textContent = `Page ${page}`;
  $("previous").disabled = page === 1;
  $("next").disabled = page * 20 >= result.total;
}
$("search-form").addEventListener("submit", async (event) => {
  event.preventDefault();

  page = 1;

  try {
    await load();
  } catch (reason) {
    failure(reason);
  }
});
for (const [id, delta] of [
  ["previous", -1],
  ["next", 1],
])
  $(id).addEventListener("click", async () => {

    page += delta;
    try {
      await load();
    } catch (reason) {
      page -= delta;
      failure(reason);
    }
  });
$("create-form").addEventListener("submit", async (event) => {
  event.preventDefault();

  event.submitter.disabled = true;
  try {
    const result = await requestJSON("/users", {
      method: "POST",
      body: {
        username: $("new-username").value,
        password: $("initial-password").value,
        role: $("role").value,
      },
    });
    $("initial-password").value = "";
    showToken(result.token, "User created. Username and role are fixed.");
    await load().catch(() => {
      notify("User list refresh failed. Use Search to try again.", true);
    });
  } catch (reason) {
    failure(reason);
  } finally {
    event.submitter.disabled = false;
  }
});
try {
  administrator = await requestJSON("/me");
  navigation(administrator);
  if (administrator.role !== "administrator")
    throw Object.assign(new Error(), { status: 403 });
  $("title").textContent = users ? "Users" : "All pages";
  $("owner-filter").hidden = users;
  $("create").hidden = !users;
  $("columns").innerHTML =
    users ? "<tr><th>ID</th><th>Username</th><th>Role</th><th>Actions</th></tr>" : "<tr><th>Slug</th><th>Actions</th><th>Last modified</th></tr>";
  $("rows").closest("table").classList.toggle("pages-table", !users);
  $("admin").hidden = false;
  await load();
  $("status").textContent = "";
} catch (reason) {
  $("status").textContent = "";
  failure(reason);
}
