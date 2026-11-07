import { requestJSON } from "/_/assets/api.js";
const $ = (id) => document.getElementById(id);
const users = location.pathname.endsWith("/users");
let page = 1;
const clearToken = () => {
  $("token").value = "";
  $("token-panel").hidden = true;
};
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
  $("error").textContent =
    messages[reason.status] || "The operation failed. Please try again.";
  if (reason.status === 401) {
    const link = document.createElement("a");
    link.href = "/_/app/login";
    link.textContent = " Log in";
    $("error").append(link);
  }
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
    clearToken();
    $("error").textContent = "";
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
function reveal(result) {
  $("token").value = result.token;
  $("token-panel").hidden = false;
  $("token").focus();
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
    const row = $("rows").insertRow();
    cell(row, record.id);
    cell(row, users ? record.username : record.slug);
    cell(row, users ? record.role : record.owner_id);
    const actions = row.insertCell();
    if (users) {
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
        reveal(result);
        $("status").textContent =
          "Password reset. All account sessions were revoked.";
      });
      button(actions, "Reset API token", async () => {
        reveal(
          await requestJSON(`/users/${record.id}/token`, { method: "POST" }),
        );
        $("status").textContent =
          "API token reset. The previous token is invalid.";
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
        $("status").textContent = "User permanently deleted.";
      });
    } else {
      for (const [name, href] of [
        ["Read", `/${record.slug}`],
        ["Edit / delete", `/_/app/pages/${record.id}/edit`],
      ]) {
        const a = document.createElement("a");
        a.textContent = name;
        a.href = href;
        actions.append(a, " ");
      }
      const label = document.createElement("label");
      label.textContent = "New owner ID";
      const owner = document.createElement("input");
      owner.type = "number";
      owner.min = "1";
      label.append(owner);
      actions.append(label);
      button(actions, "Transfer ownership", async () => {
        await requestJSON(`/pages/${record.id}/owner`, {
          method: "POST",
          body: { version: record.version, owner_id: Number(owner.value) },
        });
        await load();
        $("status").textContent =
          "Ownership transferred. Previous owner privileges and visitor access grants were revoked.";
      });
    }
  }
  $("count").textContent = `${result.total} ${users ? "users" : "pages"}`;
  $("page-number").textContent = `Page ${page}`;
  $("previous").disabled = page === 1;
  $("next").disabled = page * 20 >= result.total;
}
$("dismiss").addEventListener("click", clearToken);
window.addEventListener("pagehide", clearToken);
$("search-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  clearToken();
  page = 1;
  $("error").textContent = "";
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
    clearToken();
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
  clearToken();
  $("error").textContent = "";
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
    reveal(result);
    $("status").textContent = "User created. Username and role are fixed.";
    await load().catch(() => {
      $("error").textContent = "User list refresh failed. Use Search to try again.";
    });
  } catch (reason) {
    failure(reason);
  } finally {
    event.submitter.disabled = false;
  }
});
try {
  const me = await requestJSON("/me");
  if (me.role !== "administrator")
    throw Object.assign(new Error(), { status: 403 });
  $("title").textContent = users ? "Users" : "All pages";
  $("owner-filter").hidden = users;
  $("create").hidden = !users;
  $("columns").innerHTML =
    `<tr><th>ID</th><th>${users ? "Username" : "Slug"}</th><th>${users ? "Role" : "Owner ID"}</th><th>Actions</th></tr>`;
  $("admin").hidden = false;
  await load();
  $("status").textContent = "";
} catch (reason) {
  $("status").textContent = "";
  failure(reason);
}
