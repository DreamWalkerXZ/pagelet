import { renderMarkdown } from "./renderer.js";

const slug = location.pathname.slice(1);
const endpoint = `/_/api/v1/pages/by-slug/${encodeURIComponent(slug)}`;
const status = document.querySelector("#status");
const article = document.querySelector("article");
const challenge = document.querySelector("#challenge");
const form = document.querySelector("form");
const password = document.querySelector("#password");
const error = document.querySelector("#password-error");
const raw = document.querySelector("#raw");
let loading = false;

async function read() {
  if (loading) return;
  loading = true;
  try {
    const response = await fetch(endpoint, { cache: "no-store" });
    article.hidden = true;
    raw.hidden = true;
    article.replaceChildren();
    challenge.hidden = response.status !== 401;
    status.hidden = response.status === 401;
    if (response.status === 401) {
      document.title = "Password required · Pagelet";
      password.focus();
    } else if (response.ok) {
      const page = await response.json();
      article.innerHTML = renderMarkdown(page.markdown);
      article.hidden = false;
      challenge.hidden = true;
      status.hidden = true;
      raw.href = `/_/raw/${encodeURIComponent(slug)}`;
      raw.hidden = false;
      document.title = `${article.querySelector("h1")?.textContent || slug} · Pagelet`;
    } else {
      status.hidden = false;
      status.textContent =
        response.status === 404
          ? "Page not found."
          : "Unable to load this page. Please try again.";
      document.title = "Page unavailable · Pagelet";
    }
  } catch {
    status.hidden = false;
    status.textContent = "Unable to connect. Please reload to try again.";
  } finally {
    loading = false;
  }
}

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  const button = form.querySelector("button");
  button.disabled = true;
  error.textContent = "";
  try {
    const response = await fetch(`${endpoint}/verify-password`, {
      method: "POST",
      headers: { "content-type": "application/json", "x-pagelet-csrf": "1" },
      body: JSON.stringify({ password: password.value }),
    });
    password.value = "";
    if (response.ok) await read();
    else
      error.textContent =
        response.status === 429
          ? "Too many attempts. Please try again later."
          : response.status === 401
            ? "Incorrect password. Please try again."
            : "Unable to open this page. Please try again.";
  } catch {
    error.textContent = "Unable to connect. Please try again.";
  } finally {
    button.disabled = false;
  }
});
// Revalidate server access when returning to a page without retaining hidden source.
window.addEventListener("pageshow", (event) => {
  if (event.persisted) read();
});
document.addEventListener("visibilitychange", () => {
  if (!document.hidden) read();
});
read();
