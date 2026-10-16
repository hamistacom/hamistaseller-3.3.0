// Where to search: province → «شهرستان» (or «همه شهرهای استان»), or «شهر / منطقه» for one exact
// city, district or neighbourhood. Used by the scan page and the schedule editor.
import { h, fa, clear, select, input } from "./core.js";
import { PROVINCES } from "./data.js";

const ALL = "همه شهرهای استان";

/**
 * @param {{province?:string, city?:string, district?:string}} init
 * @returns {{el:HTMLElement, value:()=>{province:string,city:string,district:string,areas:string[]}, setNeeded:(on:boolean)=>void, error:()=>string}}
 */
export function areaPicker(init = {}, { onChange } = {}) {
  let mode = init.district ? "district" : "county";
  let needed = true;
  const prov = select([["", "انتخاب استان…"], ...Object.keys(PROVINCES).map((p) => [p, p])], init.province || "");
  const county = h("select", { class: "input" });
  const district = input({ placeholder: "مثلاً تجریش، منطقه ۵، شهرک غرب", value: init.district || "" });
  const hint = h("span", { class: "hint" });
  const tabBtn = (key, label) => h("button", { type: "button", "data-k": key, onclick: () => setMode(key) }, label);
  const tabs = h("div", { class: "mini-tabs", role: "tablist" }, tabBtn("county", "شهرستان"), tabBtn("district", "شهر / منطقه"));
  const districtBox = h("div", { class: "field" }, h("label", {}, "نام شهر، منطقه یا محله"), district);

  const fillCounties = (keep) => {
    const list = PROVINCES[prov.value] || [];
    const extra = keep && !list.includes(keep) ? [keep] : []; // a city typed in an older version stays
    const first = ["", !prov.value ? "شهرستان" : mode === "district" ? "شهرستان (اختیاری)" : ALL];
    clear(county, [first, ...[...list, ...extra].map((c) => [c, c])].map(([v, l]) => h("option", { value: v, selected: v === (keep || "") }, l)));
    county.disabled = !prov.value;
  };

  const value = () => {
    const p = prov.value;
    const c = county.value;
    if (mode === "district") return { province: p, city: c, district: district.value.trim(), areas: [] };
    return { province: p, city: c, district: "", areas: p && !c ? [...(PROVINCES[p] || [])] : [] };
  };

  const describe = () => {
    const v = value();
    if (!needed) return "اختیاری؛ این منبع بر اساس موضوع جستجو می‌کند.";
    if (!v.province) return "استان را انتخاب کنید.";
    if (mode === "district") return v.district ? `فقط «${v.district}${v.city ? `، ${v.city}` : ""}» جستجو می‌شود؛ نتیجه دقیق‌تر و محدودتر.` : "نام دقیق شهر، منطقه یا محله را بنویسید.";
    if (!v.city) return `${fa(v.areas.length)} شهر استان ${v.province} یکی‌یکی جستجو می‌شوند.`;
    return `${v.city} و محله‌های اطراف آن جستجو می‌شوند.`;
  };

  const sync = () => {
    for (const b of tabs.children) {
      const on = b.dataset.k === mode;
      b.classList.toggle("on", on);
      b.setAttribute("aria-selected", on ? "true" : "false");
    }
    districtBox.hidden = mode !== "district";
    hint.textContent = describe();
    onChange?.();
  };

  function setMode(m) {
    if (m === mode) return;
    mode = m;
    fillCounties(county.value);
    sync();
    if (m === "district") district.focus();
  }

  prov.addEventListener("change", () => {
    fillCounties("");
    sync();
  });
  county.addEventListener("change", sync);
  district.addEventListener("input", sync);
  fillCounties(init.city || "");
  sync();

  const el = h(
    "div",
    { class: "area-pick stack tight" },
    h("div", { class: "row between" }, h("span", { class: "label-t" }, "محدوده جستجو"), tabs),
    h("div", { class: "grid-2" }, prov, county),
    districtBox,
    hint,
  );
  return {
    el,
    value,
    setNeeded(on) {
      needed = on;
      prov.firstChild.textContent = on ? "انتخاب استان…" : "استان (اختیاری)";
      sync();
    },
    error() {
      const v = value();
      if (!needed) return "";
      if (!v.province) return "برای منابع نقشه‌ای، استان را انتخاب کنید.";
      if (mode === "district" && !v.district) return "نام شهر یا منطقه را بنویسید، یا زبانه «شهرستان» را انتخاب کنید.";
      return "";
    },
  };
}
