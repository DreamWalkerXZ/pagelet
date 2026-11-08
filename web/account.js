import { requestJSON } from "/_/assets/api.js";
import { confirmAction, navigation, notify, pageRow, showToken } from "/_/assets/ui.js";

const byId = (id) => document.getElementById(id);
let user;
let page = 1;
let search = "";
window.addEventListener("pageshow", (event) => {
  if (event.persisted) location.reload();
});
function failure(reason) {
  notify(reason.status === 401
    ? "Your session has ended or the current password is incorrect. Log in again if needed."
    : "Could not complete the request. Please check your input and try again.", true);
}
async function pages() {
  const result = await requestJSON(
    `/pages?${new URLSearchParams({ owner_id: user.id, page, search })}`,
  );
  byId("rows").replaceChildren();
  for (const item of result.pages) pageRow(byId("rows"), item);
  byId("empty").hidden = result.total !== 0;
  byId("pagination").textContent =
    `Page ${page} of ${Math.max(1, Math.ceil(result.total / result.per_page))} · ${result.total} pages`;
  byId("previous").disabled = page === 1;
  byId("next").disabled = page * result.per_page >= result.total;
}
function listView() {
  document.title = "Pagelet · My Pages";
  byId("view").innerHTML =
    `<h1>My Pages</h1>
      <p>Find your published pages by slug.</p>
      <p id="list-loading" hidden>Loading pages…</p>
      <p id="empty" hidden>No pages found. <a href="/_/app/new">Publish A New Page</a>.</p>
      <form id="search-form">
        <label for="search">Search slugs</label>
        <div class="search-controls"><input id="search" type="search" /><button>Search</button></div>
      </form>
      <div class="table-scroll"><table class="pages-table">
        <thead><tr><th scope="col">Slug</th><th scope="col">Actions</th><th scope="col">Last Modified</th></tr></thead>
        <tbody id="rows"></tbody>
      </table></div>
      <div class="actions"><button id="previous" type="button">Previous</button><span id="pagination"></span><button id="next" type="button">Next</button></div>`;
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
  byId("list-loading").hidden = false;
  try {
    await pages();
  } catch (reason) {
    if (reason.status === 401) location.replace("/_/app/login");
    else failure(reason);
  } finally {
    byId("list-loading").hidden = true;
    byId("view").inert = false;
  }
}
function accountView() {
  byId("view").innerHTML =
    `<h1>Account</h1><p id="identity"></p><section><h2>Change Password</h2><p>Changing your password signs out every web session and replaces your API token.</p><form id="password-form"><label for="current-password">Current password</label><input id="current-password" type="password" autocomplete="current-password" required /><label for="new-password">New password</label><input id="new-password" type="password" autocomplete="new-password" required aria-describedby="password-help" /><small id="password-help">Use 12–128 characters. Spaces and Unicode are preserved.</small><button>Change Password</button></form></section><section><h2>API Token</h2><p>Resetting immediately invalidates your previous token. Your web session stays signed in.</p><button id="reset-token" type="button">Reset API Token</button></section><section><h2>Web Session</h2><button id="logout" type="button">Log Out</button></section>`;
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
      showToken(result.token, "Password changed. Save your new token before logging in again.",
        () => location.replace("/_/app/login"));
    });
  };
  byId("reset-token").onclick = async () => {
    if (!await confirmAction("Reset API Token",
      "Your previous API token will stop working immediately. Your web sessions will stay signed in.",
    )) return;
    await mutation(async () => {
      const result = await requestJSON("/me/token", { method: "POST" });
      showToken(result.token, "API token reset. Your web session remains signed in.",
        () => byId("reset-token").focus());
    });
  };
  byId("logout").onclick = () =>
    mutation(async () => {
      await requestJSON("/logout", { method: "POST" });
      location.replace("/_/app/login");
    });
}
async function mutation(action) {
  const buttons = [...byId("view").querySelectorAll("button")];
  buttons.forEach((button) => {
    button.disabled = true;
  });
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
  navigation(user);
  if (location.pathname === "/_/app/pages") await listView();
  else accountView();
  byId("status").textContent = "";
} catch (reason) {
  if (reason.status === 401) location.replace("/_/app/login");
  else failure(reason);
  byId("status").textContent = "";
}
