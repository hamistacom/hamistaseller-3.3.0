// App-wide state shared by pages.
import { invoke } from "./core.js";

export const app = {
  info: null, // app_info: {version, edition, dataDir, license, migrated}
  lists: [], // banks with counts
  handoff: null, // {filter, label} passed from the contacts page to campaigns / assistant
};

export const bus = new EventTarget();
export const emit = (name, detail) => bus.dispatchEvent(new CustomEvent(name, { detail }));
export const on = (name, fn) => {
  const f = (e) => fn(e.detail);
  bus.addEventListener(name, f);
  return () => bus.removeEventListener(name, f);
};

export async function refreshLists() {
  app.lists = await invoke("lists");
  emit("lists", app.lists);
  return app.lists;
}

export async function refreshLicense() {
  app.info.license = await invoke("license_status");
  emit("license", app.info.license);
  return app.info.license;
}

export const listName = (id) => app.lists.find((l) => l.id === Number(id))?.name || "";

/** Bank <select> options, optionally with «همه بانک‌ها». */
export const bankOptions = (all = false) => [...(all ? [[0, "همه بانک‌ها"]] : []), ...app.lists.map((l) => [l.id, l.name])];

/** Navigate to a page: go("contacts", {list: 2, q: "کافه"}). */
export function go(name, params = {}) {
  const q = new URLSearchParams(Object.entries(params).filter(([, v]) => v !== "" && v != null)).toString();
  const target = `#/${name}${q ? `?${q}` : ""}`;
  if (location.hash === target) window.dispatchEvent(new Event("reroute"));
  else location.hash = target;
}
