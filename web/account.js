import { requestJSON } from "/_/assets/api.js";

const byId = (id) => document.getElementById(id);
let user;
let page = 1;
let search = "";
function failure(reason) {
  byId("error").textContent =
    reason.status === 401
      ? "Your session has ended or the current password is incorrect. Log in again if needed."
      : "Could not complete the request. Please check your input and try again.";
  byId("reauth").hidden = reason.status !== 401;
}
function reveal(token) {
  byId("secret").hidden = false;
  byId("token").value = token;
  byId("token").focus();
  byId("token").select();
}
function clearToken() {
  byId("token").value = "";
  byId("secret").hidden = true;
}
byId("dismiss").onclick = clearToken;
window.addEventListener("pagehide", clearToken);
window.addEventListener("pageshow", (event) => {
  if (event.persisted) location.reload();
});
async function pages() {
  const result = await requestJSON(
    `/pages?${new URLSearchParams({ owner_id: user.id, page, search })}`,
  );
  byId("rows").replaceChildren();
  for (const item of result.pages) {
    const row = document.createElement("li");
    const read = document.createElement("a");
    read.href = `/${item.slug}`;
    read.textContent = item.slug;
    read.setAttribute("aria-label", `Read ${item.slug}`);
    const edit = document.createElement("a");
    edit.href = `/_/app/pages/${item.id}/edit`;
    edit.textContent = "Edit";
    edit.setAttribute("aria-label", `Edit ${item.slug}`);
    const date = document.createElement("small");
    date.textContent = `Updated ${new Date(item.updated_at).toLocaleString()}`;
    row.append(read, " · ", edit, date);
    byId("rows").append(row);
  }
  byId("empty").hidden = result.total !== 0;
  byId("pagination").textContent =
    `Page ${page} of ${Math.max(1, Math.ceil(result.total / result.per_page))} · ${result.total} pages`;
  byId("previous").disabled = page === 1;
  byId("next").disabled = page * result.per_page >= result.total;
}
function listView() {
  document.title = "Pagelet · My pages";
  byId("view").innerHTML =
    `<h1>My pages</h1><p>Find your published pages by slug.</p><form id="search-form"><label for="search">Search slugs</label><input id="search" type="search" /><button>Search</button></form><p id="empty" hidden>No pages found. <a href="/_/app/new">Publish a new page</a>.</p><ul id="rows"></ul><div class="actions"><button id="previous" type="button">Previous</button><span id="pagination"></span><button id="next" type="button">Next</button></div>`;
  byId("search-form").onsubmit = async (event) => {
    event.preventDefault();
    search = byId("search").value;
    page = 1;
    await loadPages();
  };
  for (const [id, delta] of [
    ["previous", -1],
    ["next", 1],
  ]) {
    byId(id).onclick = async () => {
      page += delta;
      await loadPages();
    };
  }
  return loadPages();
}
async function loadPages() {
  byId("view").inert = true;
  byId("error").textContent = "";
  try {
    await pages();
  } catch (reason) {
    failure(reason);
  } finally {
    byId("view").inert = false;
  }
}
function accountView() {
  byId("view").innerHTML =
    `<h1>Account</h1><p id="identity"></p><section><h2>Change password</h2><p>Changing your password signs out every web session and replaces your API token.</p><form id="password-form"><label for="current-password">Current password</label><input id="current-password" type="password" autocomplete="current-password" required /><label for="new-password">New password</label><input id="new-password" type="password" autocomplete="new-password" required aria-describedby="password-help" /><small id="password-help">Use 12–128 characters. Spaces and Unicode are preserved.</small><button>Change password</button></form></section><section><h2>API token</h2><p>Resetting immediately invalidates your previous token. Your web session stays signed in.</p><button id="reset-token" type="button">Reset API token</button></section><section><h2>Web session</h2><button id="logout" type="button">Log out</button></section>`;
  byId("identity").textContent = `Signed in as ${user.username}.`;
  byId("password-form").onsubmit = async (event) => {
    event.preventDefault();
    await mutation(async () => {
      const result = await requestJSON("/me/password", {
        method: "POST",
        body: {
          current_password: byId("current-password").value,
          password: byId("new-password").value,
        },
      });
      byId("password-form").reset();
      byId("view").inert = true;
      byId("reauth").hidden = false;
      byId("status").textContent =
        "Password changed. All web sessions have ended. Save your new token, then log in again.";
      reveal(result.token);
    });
  };
  byId("reset-token").onclick = () =>
    mutation(async () => {
      const result = await requestJSON("/me/token", { method: "POST" });
      byId("status").textContent =
        "API token reset. Your web session remains signed in.";
      reveal(result.token);
    });
  byId("logout").onclick = () =>
    mutation(async () => {
      await requestJSON("/logout", { method: "POST" });
      clearToken();
      location.replace("/_/app/login");
    });
}
async function mutation(action) {
  const buttons = [...byId("view").querySelectorAll("button")];
  buttons.forEach((button) => {
    button.disabled = true;
  });
  byId("error").textContent = "";
  try {
    await action();
  } catch (reason) {
    failure(reason);
  } finally {
    buttons.forEach((button) => {
      button.disabled = false;
    });
  }
}
try {
  user = await requestJSON("/me");
  if (user.role === "administrator") {
    for (const [path, label] of [
      ["pages", "All pages"],
      ["users", "Users"],
    ]) {
      const link = document.createElement("a");
      link.href = `/_/app/admin/${path}`;
      link.textContent = label;
      document.querySelector("nav").append(link);
    }
  }
  if (location.pathname === "/_/app/pages") await listView();
  else accountView();
  byId("status").textContent = "";
} catch (reason) {
  failure(reason);
  byId("status").textContent = "";
}
