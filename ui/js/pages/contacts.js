// Banks + contacts table: filters, full names with manager under the brand, bulk actions, import / export.
import { h, icon, invoke, fa, clear, safe, select, field, input, toast, modal, confirmBox, copy, handle, fileToBase64, pickFile, bar, appBadges, vtable, KIND, STATUS, COLORS } from "../core.js";
import { app, bankOptions, refreshLists, listName, go, on } from "../state.js";
import { openCard, openEditor } from "./card.js";
import { newBankModal } from "./scan.js";

const PAGE = 100;
const CHANNELS = [
  ["", "همه راه‌ها"],
  ["mobile", "دارای موبایل"],
  ["landline", "دارای تلفن ثابت"],
  ["email", "دارای ایمیل"],
  ["website", "دارای سایت"],
  ["person", "دارای نام مسئول"],
  ["social", "دارای شبکه اجتماعی"],
  ["instagram", "اینستاگرام"],
  ["telegram", "تلگرام"],
  ["whatsapp", "واتساپ"],
  ["messenger", "ایتا / بله"],
];

let st = null; // page state
let offLists = null;

export async function render(root, params, actions) {
  st = {
    listId: Number(params.list) || 0,
    q: params.q || "",
    channel: params.channel || "",
    status: params.status || "",
    keyword: params.keyword || "",
    city: params.city || "",
    province: params.province || "",
    category: params.category || "",
    tag: params.tag || "",
    view: params.view || remember("contactsView") || "list", // list | stats
    offset: 0,
    sel: new Set(),
    allMatching: false,
    rows: [],
    total: 0,
  };
  actions.append(
    h("button", { class: "btn", type: "button", onclick: importFlow }, icon("upload", "sm"), "ورود فایل"),
    h("button", { class: "btn", type: "button", onclick: exportMenu }, icon("download", "sm"), "خروجی"),
    h("button", { class: "btn primary", type: "button", onclick: () => openEditor({}, { listId: st.listId || undefined, onChange: () => load() }) }, icon("plus", "sm"), "افزودن مخاطب"),
  );
  st.banksEl = h("div", { class: "banks" });
  st.filtersEl = h("div", { class: "filters" });
  st.viewEl = h("div", { class: "row between wrap" });
  st.tableEl = h("div", { class: "card flush" });
  st.bulkEl = h("div", { class: "bulk", hidden: true });
  root.append(
    h(
      "div",
      { class: "split" },
      h(
        "div",
        { class: "card banks-card" },
        h("div", { class: "card-h" }, h("h2", {}, "بانک‌ها")),
        st.banksEl,
        h("button", { class: "bank-new", type: "button", onclick: () => newBankModal((id) => selectBank(id)) }, icon("plus", "sm"), "بانک جدید"),
      ),
      h("div", { class: "stack" }, h("div", { class: "card stack tight" }, st.viewEl, st.filtersEl), st.tableEl, st.bulkEl),
    ),
  );
  offLists = on("lists", drawBanks);
  drawBanks();
  await drawFilters();
  await load();
  if (params.import) importFlow();
}

export function leave() {
  offLists?.();
}

// ---------------- banks ----------------

function drawBanks() {
  const total = app.lists.reduce((a, l) => a + l.count, 0);
  const item = (id, name, count, color, l) =>
    h(
      "div",
      { class: `bank ${st.listId === id ? "on" : ""}`, onclick: () => selectBank(id) },
      h("span", { class: `sw ${color ? `c-${color}` : ""}` }),
      h("span", { class: "nm", title: name }, name),
      h("span", { class: "ct" }, fa(count)),
      l ? h("button", { class: "btn ghost sm icon-only more", type: "button", title: "ویرایش بانک", onclick: (e) => { e.stopPropagation(); editBank(l); } }, icon("edit", "sm")) : null,
    );
  clear(st.banksEl, item(0, "همه بانک‌ها", total, ""), app.lists.map((l) => item(l.id, l.name, l.count, l.color || "navy", l)));
}

async function selectBank(id) {
  st.listId = id;
  st.offset = 0;
  st.sel.clear();
  st.allMatching = false;
  st.keyword = st.city = st.tag = st.province = st.category = "";
  drawBanks();
  await drawFilters();
  await load();
}

function editBank(l) {
  const name = input({ value: l.name });
  const note = h("textarea", { class: "input", rows: 2 }, l.note || "");
  let color = l.color || "navy";
  const sw = h("div", { class: "chips" }, COLORS.map((c) => h("span", { class: `chip click ${c === color ? "on" : ""}`, onclick: (e) => { color = c; [...sw.children].forEach((x) => x.classList.remove("on")); e.currentTarget.classList.add("on"); } }, h("span", { class: `sw c-${c} bank-sw` }))));
  const m = modal({
    title: "ویرایش بانک",
    body: [field("نام", name), field("رنگ", sw), field("توضیح", note)],
    left: h("button", {
      class: "btn danger sm",
      type: "button",
      onclick: async () => {
        if (!(await confirmBox(`بانک «${l.name}» و ${fa(l.count)} مخاطب آن حذف شود؟ این کار برگشت ندارد (از پشتیبان روزانه قابل بازگردانی است).`, { ok: "حذف بانک", danger: true }))) return;
        if ((await safe(invoke("list_delete", { id: l.id }), "بانک حذف شد")) !== undefined) {
          m.close();
          await refreshLists();
          selectBank(0);
        }
      },
    }, icon("trash", "sm"), "حذف بانک"),
    actions: [
      { label: "انصراف" },
      { label: "ذخیره", kind: "primary", onClick: async () => ((await safe(invoke("list_update", { id: l.id, name: name.value, color, note: note.value }), "ذخیره شد")) === undefined ? false : refreshLists()) },
    ],
  });
}

// ---------------- filters ----------------

let qTimer = 0;
async function drawFilters() {
  const f = await invoke("facets", { listId: st.listId });
  const opt = (all, rows) => [["", all], ...rows.map(([v, n]) => [v, `${v} (${fa(n)})`])];
  const q = input({ class: "input search-in", placeholder: "جستجو در نام، شماره، مسئول، آدرس، یادداشت…", value: st.q });
  q.addEventListener("input", () => {
    clearTimeout(qTimer);
    qTimer = setTimeout(() => set({ q: q.value.trim() }), 300);
  });
  const sel = (key, options) => select(options, st[key], { class: "input", onchange: (e) => set({ [key]: e.target.value }) });
  const any = FILTER_KEYS.some((k) => st[k]);
  const keep = (rows, v) => (v && !rows.some(([x]) => x === v) ? [...rows, [v, 0]] : rows); // a drilled-down value stays selectable
  clear(
    st.filtersEl,
    q,
    sel("channel", CHANNELS),
    sel("status", [["", "همه وضعیت‌ها"], ...Object.entries(STATUS).map(([k, v]) => [k, v.label])]),
    f.provinces.length ? sel("province", opt("همه استان‌ها", keep(f.provinces, st.province))) : null,
    f.cities.length ? sel("city", opt(st.province ? "همه شهرهای استان" : "همه شهرستان‌ها", keep(f.cities, st.city))) : null,
    f.categories.length ? sel("category", opt("همه حوزه‌های کاری", keep(f.categories, st.category))) : null,
    f.keywords.length ? sel("keyword", opt("همه کلمات", keep(f.keywords, st.keyword))) : null,
    f.tags.length ? sel("tag", opt("همه برچسب‌ها", keep(f.tags, st.tag))) : null,
    any ? h("button", { class: "btn ghost sm", type: "button", onclick: () => { FILTER_KEYS.forEach((k) => (st[k] = "")); drawFilters(); set({}); } }, icon("x", "sm"), "حذف فیلترها") : null,
  );
  drawView();
}

const FILTER_KEYS = ["q", "channel", "status", "province", "city", "category", "keyword", "tag"];

function remember(k, v) {
  try {
    if (v === undefined) return localStorage.getItem(k);
    localStorage.setItem(k, v);
  } catch {}
  return null;
}

/** «فهرست مخاطبان» / «تحلیل و تفکیک» switch. */
function drawView() {
  const b = (v, label, ic) => h("button", { class: st.view === v ? "on" : "", type: "button", onclick: () => { st.view = v; remember("contactsView", v); drawView(); load(); } }, icon(ic, "sm"), label);
  clear(st.viewEl, h("div", { class: "seg" }, b("list", "فهرست مخاطبان", "bank"), b("stats", "تحلیل و تفکیک", "dashboard")), st.view === "stats" ? h("span", { class: "small muted" }, "روی هر مورد بزنید تا همان‌جا ریزتر شوید؛ فیلترها روی خروجی و کمپین هم اعمال می‌شوند.") : null);
}

function set(patch) {
  Object.assign(st, patch, { offset: 0, allMatching: false });
  st.sel.clear();
  load();
}

const filter = (ids) => ({ listId: st.listId, q: st.q, channel: st.channel, status: st.status, keyword: st.keyword, province: st.province, city: st.city, category: st.category, tag: st.tag, ids: ids || [] });

// ---------------- table ----------------

let loadSeq = 0; // only the latest request may draw (fast typing, view switches)
async function load() {
  const my = ++loadSeq;
  if (st.view === "stats") return loadStats(my);
  const r = await safe(invoke("places", { filter: filter(), offset: st.offset, limit: PAGE }));
  if (!r || my !== loadSeq) return;
  if (!r.rows.length && st.offset > 0) {
    // the last page emptied (delete / move): show the new last page
    st.offset = Math.max(0, (Math.ceil(r.total / PAGE) - 1) * PAGE);
    return load();
  }
  st.tableEl.className = "card flush";
  st.rows = r.rows;
  st.total = r.total;
  drawTable();
  drawBulk();
}

function channelIcons(p) {
  return h(
    "div",
    { class: "ch-icons" },
    p.contacts
      .filter((c) => c.kind !== "mobile" && c.kind !== "landline")
      .map((c) => {
        const link = c.value.startsWith("http");
        return h("button", { class: `ch k-${c.kind}`, type: "button", title: `${KIND[c.kind]?.label}: ${link ? handle(c.value) : c.value}`, onclick: () => (link ? safe(invoke("open_url", { url: c.value })) : copy(c.value, `${c.value} کپی شد`)) }, icon(KIND[c.kind]?.icon || "info"));
      }),
    p.website ? h("button", { class: "ch k-website", type: "button", title: `سایت: ${p.website}`, onclick: () => safe(invoke("open_url", { url: p.website })) }, icon("globe")) : null,
  );
}

/** Numbers of one kind (copy on click); mobiles carry the messengers seen for them. */
function numbers(list) {
  if (!list.length) return h("span", { class: "x" }, "—");
  return h(
    "div",
    { class: "phones" },
    list.slice(0, 2).map((c) => h("span", { class: "num" }, h("span", { class: "ltr", title: "کپی", onclick: () => copy(c.value) }, c.value), appBadges(c))),
    list.length > 2 ? h("span", { class: "x" }, `+${fa(list.length - 2)} شماره دیگر`) : null,
  );
}

function row(p, i) {
  const checked = st.allMatching || st.sel.has(p.id);
  const cb = h("input", { type: "checkbox", checked, "aria-label": "انتخاب" });
  cb.addEventListener("change", () => {
    if (st.allMatching) {
      // leaving «همه»: keep exactly what this page shows as checked
      st.allMatching = false;
      st.sel.clear();
      st.rows.forEach((r) => st.sel.add(r.id));
      cb.checked ? st.sel.add(p.id) : st.sel.delete(p.id);
      drawTable();
      drawBulk();
      return;
    }
    cb.checked ? st.sel.add(p.id) : st.sel.delete(p.id);
    tr.classList.toggle("sel", cb.checked);
    drawBulk();
    drawHead();
  });
  const mobiles = p.contacts.filter((c) => c.kind === "mobile");
  const landlines = p.contacts.filter((c) => c.kind === "landline");
  const status = h(
    "select",
    { class: `pill ${STATUS[p.status]?.tone || ""}`, "aria-label": "وضعیت" },
    Object.entries(STATUS).map(([k, v]) => h("option", { value: k, selected: k === p.status }, v.label)),
  );
  status.addEventListener("change", async () => {
    if ((await safe(invoke("place_status", { id: p.id, status: status.value, note: p.note }))) === undefined) return (status.value = p.status);
    p.status = status.value;
    status.className = `pill ${STATUS[p.status]?.tone || ""}`;
  });
  const open = () => openCard(p.id, { onChange: () => load() });
  const tr = h(
    "tr",
    { class: `click ${checked ? "sel" : ""}`.trim() },
    h("td", {}, cb),
    h("td", { class: "rn" }, fa(st.offset + i + 1)),
    h(
      "td",
      {},
      h(
        "div",
        { class: "name-cell" },
        h("span", { class: "n", title: p.name }, p.name),
        p.contactPerson ? h("span", { class: "p" }, icon("user", "sm"), p.contactPerson, p.contactRole ? ` — ${p.contactRole}` : "") : null,
        h("span", { class: "c" }, [p.category, p.city, st.listId ? "" : listName(p.listId)].filter(Boolean).join(" · ")),
      ),
    ),
    h("td", {}, numbers(mobiles)),
    h("td", {}, numbers(landlines)),
    h("td", { class: "hide-md" }, channelIcons(p)),
    h("td", {}, status),
  );
  // the whole row opens the card (checkbox, status, numbers and icons keep their own action)
  tr.addEventListener("click", (e) => {
    if (!e.target.closest("input, select, button, .num .ltr")) open();
  });
  return tr;
}

function drawHead() {
  const pageIds = st.rows.map((r) => r.id);
  const all = st.allMatching || (pageIds.length > 0 && pageIds.every((id) => st.sel.has(id)));
  if (st.headCb) {
    st.headCb.checked = all;
    st.headCb.indeterminate = !all && pageIds.some((id) => st.sel.has(id));
  }
}

function drawTable() {
  if (!st.total) {
    clear(
      st.tableEl,
      h(
        "div",
        { class: "empty" },
        icon("bank", "lg"),
        st.q || st.channel || st.status || st.keyword || st.city || st.tag ? "مخاطبی با این فیلترها پیدا نشد." : "این بانک هنوز خالی است.",
        h("div", { class: "row" }, h("button", { class: "btn accent sm", type: "button", onclick: () => go("scan") }, icon("play", "sm"), "استخراج"), h("button", { class: "btn sm", type: "button", onclick: importFlow }, icon("upload", "sm"), "ورود فایل")),
      ),
    );
    return;
  }
  st.headCb = h("input", { type: "checkbox", "aria-label": "انتخاب همه" });
  st.headCb.addEventListener("change", () => {
    if (st.allMatching) st.sel.clear();
    st.allMatching = false;
    st.rows.forEach((r) => (st.headCb.checked ? st.sel.add(r.id) : st.sel.delete(r.id)));
    drawTable();
    drawBulk();
  });
  const pages = Math.ceil(st.total / PAGE);
  const page = Math.floor(st.offset / PAGE) + 1;
  clear(
    st.tableEl,
    vtable({
      cols: h(
        "colgroup",
        {},
        h("col", { class: "w-check" }),
        h("col", { class: "c-rn" }),
        h("col", { class: "c-name" }),
        h("col", { class: "c-phone" }),
        h("col", { class: "c-land" }),
        h("col", { class: "c-ch hide-md" }),
        h("col", { class: "c-status" }),
      ),
      head: h("thead", {}, h("tr", {}, h("th", {}, st.headCb), h("th", {}, "#"), h("th", {}, "کسب‌وکار / مسئول"), h("th", {}, "موبایل"), h("th", {}, "تلفن ثابت"), h("th", { class: "hide-md" }, "راه‌های دیگر"), h("th", {}, "وضعیت"))),
      rows: st.rows,
      row,
      rowHeight: 58,
    }).el,
    h(
      "div",
      { class: "pager" },
      h("span", {}, `${fa(st.offset + 1)}–${fa(Math.min(st.offset + PAGE, st.total))} از ${fa(st.total)} مخاطب`),
      h(
        "div",
        { class: "row" },
        h("button", { class: "btn sm", type: "button", disabled: page <= 1, onclick: () => { st.offset -= PAGE; load(); } }, "قبلی"),
        h("span", {}, `صفحه ${fa(page)} از ${fa(pages)}`),
        h("button", { class: "btn sm", type: "button", disabled: page >= pages, onclick: () => { st.offset += PAGE; load(); } }, "بعدی"),
      ),
    ),
  );
  drawHead();
}

// ---------------- bulk actions ----------------

const selCount = () => (st.allMatching ? st.total : st.sel.size);

/** Every selected id (all pages when «همه» is chosen). */
async function selectedIds() {
  if (!st.allMatching) return [...st.sel];
  const ids = [];
  for (let off = 0; off < st.total; off += 500) {
    const r = await invoke("places", { filter: filter(), offset: off, limit: 500 });
    ids.push(...r.rows.map((p) => p.id));
  }
  return ids;
}

function drawBulk() {
  const n = selCount();
  st.bulkEl.hidden = !n;
  if (!n) return;
  const pageAll = st.rows.length && st.rows.every((r) => st.sel.has(r.id));
  const status = select([["", "تغییر وضعیت…"], ...Object.entries(STATUS).map(([k, v]) => [k, v.label])], "", { class: "input sm" });
  status.addEventListener("change", async () => {
    if (!status.value) return;
    const c = await safe(invoke("places_status", { ids: await selectedIds(), status: status.value }), (k) => `وضعیت ${fa(k)} مخاطب تغییر کرد`);
    if (c !== undefined) load();
  });
  const handoff = (page) => async () => {
    const ids = await selectedIds();
    app.handoff = { filter: { ...filter(ids) }, label: `${fa(ids.length)} مخاطب انتخاب‌شده` };
    go(page);
  };
  clear(
    st.bulkEl,
    h("b", {}, `${fa(n)} انتخاب`),
    pageAll && !st.allMatching && st.total > st.rows.length ? h("button", { class: "btn sm", type: "button", onclick: () => { st.allMatching = true; drawTable(); drawBulk(); } }, `انتخاب همه ${fa(st.total)} مورد`) : null,
    status,
    h("button", { class: "btn sm", type: "button", onclick: tagSelected }, icon("tag", "sm"), "برچسب"),
    h("button", { class: "btn sm", type: "button", onclick: transferSelected }, icon("move", "sm"), "کپی / انتقال"),
    h("button", { class: "btn sm", type: "button", onclick: handoff("outreach") }, icon("send", "sm"), "کمپین"),
    h("button", { class: "btn sm", type: "button", onclick: handoff("assist") }, icon("chat", "sm"), "دستیار گفتگو"),
    h("button", { class: "btn sm", type: "button", onclick: exportMenu }, icon("download", "sm"), "خروجی"),
    h("button", { class: "btn sm", type: "button", onclick: deleteSelected }, icon("trash", "sm"), "حذف"),
    h("span", { class: "grow" }),
    h("button", { class: "btn sm icon-only", type: "button", "aria-label": "لغو انتخاب", onclick: () => { st.sel.clear(); st.allMatching = false; drawTable(); drawBulk(); } }, icon("x", "sm")),
  );
}

function tagSelected() {
  const tag = input({ placeholder: "مثلاً VIP یا تماس دوم" });
  modal({
    title: `برچسب برای ${fa(selCount())} مخاطب`,
    body: field("برچسب", tag),
    actions: [{ label: "انصراف" }, { label: "افزودن برچسب", kind: "primary", onClick: async () => ((await safe(invoke("places_tag", { ids: await selectedIds(), tag: tag.value }), "برچسب اضافه شد")) === undefined ? false : (drawFilters(), load())) }],
  });
  tag.focus();
}

function transferSelected() {
  const to = select(bankOptions().filter(([id]) => id !== st.listId), "");
  const mode = select([["copy", "کپی (در هر دو بانک بماند)"], ["move", "انتقال (از این بانک برداشته شود)"]], "copy");
  if (!to.options.length) return toast("ابتدا یک بانک دیگر بسازید.", "bad");
  modal({
    title: `کپی یا انتقال ${fa(selCount())} مخاطب`,
    body: [field("بانک مقصد", to), field("نوع", mode, "راه‌های ارتباطی که در بانک مقصد وجود دارند، با مخاطب موجود ادغام می‌شوند.")],
    actions: [
      { label: "انصراف" },
      {
        label: "انجام",
        kind: "primary",
        onClick: async () => {
          const n = await safe(invoke("transfer", { ids: await selectedIds(), to: Number(to.value), copy: mode.value === "copy" }), (k) => `${fa(k)} مخاطب ${mode.value === "copy" ? "کپی" : "منتقل"} شد`);
          if (n === undefined) return false;
          st.sel.clear();
          st.allMatching = false;
          await refreshLists();
          load();
        },
      },
    ],
  });
}

async function deleteSelected() {
  if (!(await confirmBox(`${fa(selCount())} مخاطب حذف شوند؟`, { ok: "حذف", danger: true }))) return;
  const n = await safe(invoke("places_delete", { ids: await selectedIds() }), (k) => `${fa(k)} مخاطب حذف شد`);
  if (n === undefined) return;
  st.sel.clear();
  st.allMatching = false;
  await refreshLists();
  load();
}

// ---------------- import / export ----------------

function exportMenu() {
  const n = selCount();
  const scope = n ? `${fa(n)} مخاطب انتخاب‌شده` : `${fa(st.total)} مخاطب (فیلتر فعلی${st.listId ? ` — ${listName(st.listId)}` : ""})`;
  const run = (format) => async () => {
    const ids = n ? await selectedIds() : [];
    const path = await safe(invoke("export_data", { filter: filter(ids), format }));
    if (!path) return false;
    toast("خروجی ذخیره شد.", "ok");
    invoke("reveal", { path }).catch(() => {});
  };
  let m;
  const opt = (format, title, sub, ic) => {
    const b = h("button", { class: "btn block export-opt", type: "button" }, icon(ic, "sm"), h("span", { class: "grow" }, h("b", {}, title), h("span", { class: "muted small" }, ` — ${sub}`)));
    b.addEventListener("click", async () => {
      b.disabled = true;
      if ((await run(format)()) !== false) m.close();
      b.disabled = false;
    });
    return b;
  };
  m = modal({
    title: "خروجی",
    sub: scope,
    body: h(
      "div",
      { class: "stack tight" },
      opt("xlsx", "Excel", "هر راه ارتباطی در ستون جدا، با نام و سمت مسئول", "download"),
      opt("csv", "CSV", "برای نرم‌افزارهای دیگر", "download"),
      opt("vcf", "vCard", "ذخیره در مخاطبان گوشی", "user"),
      opt("txt", "لیست موبایل‌ها", "یک شماره در هر خط، برای پنل‌های پیامکی", "mobile"),
      h("p", { class: "small faint" }, "فایل در پوشه Downloads\\HamistaSeller ذخیره و نمایش داده می‌شود."),
    ),
  });
}

function importFlow() {
  const bank = select(bankOptions(), st.listId || app.lists[0]?.id);
  modal({
    title: "ورود از Excel یا CSV",
    body: [
      h("p", { class: "muted" }, "ردیف اول فایل باید عنوان ستون‌ها باشد. ستون‌هایی مثل «نام»، «موبایل»، «تلفن»، «ایمیل»، «اینستاگرام»، «نام مسئول»، «شهر» و «برچسب» خودکار شناخته می‌شوند. خروجی Excel خود برنامه هم مستقیم وارد می‌شود."),
      field("بانک مقصد", bank),
    ],
    actions: [
      { label: "انصراف" },
      {
        label: "انتخاب فایل",
        kind: "primary",
        icon: "upload",
        onClick: async () => {
          const file = await pickFile(".xlsx,.xls,.xlsm,.ods,.csv,.txt,.tsv");
          if (!file) return false;
          if (file.size > 20 * 1024 * 1024) return toast("فایل بزرگ‌تر از ۲۰ مگابایت است.", "bad"), false;
          const data = await fileToBase64(file);
          const rep = await safe(invoke("import_data", { listId: Number(bank.value), fileName: file.name, data }));
          if (!rep) return false;
          await refreshLists();
          selectBank(Number(bank.value));
          modal({
            title: "نتیجه ورود فایل",
            body: [
              h("div", { class: "grid-3" }, h("div", { class: "stat" }, h("b", {}, fa(rep.added)), h("span", {}, "مخاطب جدید")), h("div", { class: "stat" }, h("b", {}, fa(rep.duplicates)), h("span", {}, "تکراری")), h("div", { class: "stat" }, h("b", {}, fa(rep.skipped)), h("span", {}, "بدون راه ارتباطی"))),
              h("div", { class: "small" }, h("b", {}, "ستون‌های شناخته‌شده: "), rep.columns.join("، ") || "—"),
              rep.ignored.length ? h("div", { class: "small muted" }, h("b", {}, "ستون‌های نادیده‌گرفته‌شده: "), rep.ignored.join("، ")) : null,
              rep.truncated ? h("div", { class: "callout" }, icon("alert"), `${fa(rep.truncated)} ردیف بیش از سقف ۵۰٬۰۰۰ ردیف بود و خوانده نشد؛ فایل را به چند بخش تقسیم کنید.`) : null,
            ],
            actions: [{ label: "بستن", kind: "primary" }],
          });
        },
      },
    ],
  });
}

// ---------------- analysis: province / county / line of business ----------------

const CH_LABEL = { ...Object.fromEntries(Object.entries(KIND).map(([k, v]) => [k, v.label])), website: "وب‌سایت", person: "نام مسئول" };

/** Ranked breakdown card. rows: [{label, sub, n, pick}] */
function breakdown(title, rows, total, { empty = "داده‌ای نیست", limit = 10, ic } = {}) {
  const body = h("div", { class: "rank" });
  let all = false;
  const max = Math.max(1, ...rows.map((r) => r.n));
  const draw = () =>
    clear(
      body,
      rows.length ? null : h("div", { class: "empty small" }, empty),
      (all ? rows : rows.slice(0, limit)).map((r) =>
        h(
          "div",
          { class: "r" },
          r.pick ? h("button", { class: "link clip", type: "button", title: `فقط ${r.label}`, onclick: r.pick }, r.label, r.sub ? h("span", { class: "muted small" }, ` — ${r.sub}`) : null) : h("span", { class: "clip muted" }, r.label),
          h("span", { class: "muted small nowrap" }, `${fa(r.n)} · ${fa(total ? Math.round((r.n / total) * 100) : 0)}٪`),
          bar(r.n / max),
        ),
      ),
      rows.length > limit ? h("button", { class: "link small", type: "button", onclick: () => { all = !all; draw(); } }, all ? "نمایش کمتر" : `نمایش همه (${fa(rows.length)})`) : null,
    );
  draw();
  return h("div", { class: "card" }, h("div", { class: "card-h" }, ic ? icon(ic, "sm") : null, h("h2", {}, title), h("span", { class: "muted" }, fa(rows.length))), body);
}

async function loadStats(my) {
  st.bulkEl.hidden = true;
  const s = await safe(invoke("bank_stats", { filter: filter() }));
  if (!s || my !== loadSeq) return;
  st.tableEl.className = ""; // the analysis is its own set of cards
  const t = s.total;
  st.total = t;
  st.sel.clear();
  st.allMatching = false;
  const drill = (patch) => async () => {
    Object.assign(st, patch, { offset: 0 });
    await drawFilters();
    load();
  };
  const unknown = (rows) => t - rows.reduce((a, r) => a + r.n, 0);
  const prov = s.provinces.map(([v, n]) => ({ label: v, n, pick: drill({ province: v, city: "" }) }));
  if (unknown(prov) > 0) prov.push({ label: "استان نامشخص", n: unknown(prov) });
  const cities = s.cities.map(([p, c, n]) => ({ label: c, sub: p, n, pick: drill({ province: p, city: c }) }));
  const cats = s.categories.map(([v, n]) => ({ label: v, n, pick: drill({ category: v }) }));
  if (unknown(cats) > 0) cats.push({ label: "حوزه نامشخص", n: unknown(cats) });
  const kws = s.keywords.map(([v, n]) => ({ label: v, n, pick: drill({ keyword: v }) }));
  const stats = s.statuses.map(([v, n]) => ({ label: STATUS[v]?.label || v, n, pick: drill({ status: v }) }));
  const chans = s.channels.map(([k, n]) => ({ label: CH_LABEL[k] || k, n, pick: ["website", "person"].includes(k) || KIND[k] ? drill({ channel: k }) : null }));
  const ch = Object.fromEntries(s.channels);
  const scope = [st.listId ? listName(st.listId) : "همه بانک‌ها", st.province, st.city, st.category, st.keyword, STATUS[st.status]?.label].filter(Boolean).join(" ← ");
  clear(
    st.tableEl,
    h(
      "div",
      { class: "stack" },
      h(
        "div",
        { class: "row between wrap" },
        h("div", {}, h("h2", {}, scope), h("span", { class: "small muted" }, `${fa(t)} کسب‌وکار در این بخش`)),
        h("div", { class: "row" }, h("button", { class: "btn sm", type: "button", onclick: exportMenu }, icon("download", "sm"), "خروجی همین بخش"), h("button", { class: "btn sm primary", type: "button", onclick: () => { st.view = "list"; remember("contactsView", "list"); drawView(); load(); } }, icon("bank", "sm"), "فهرست همین بخش")),
      ),
      h(
        "div",
        { class: "grid-4" },
        [["دارای موبایل", ch.mobile], ["دارای ایمیل", ch.email], ["دارای سایت", ch.website], ["دارای نام مسئول", ch.person]].map(([l, n]) =>
          h("div", { class: "stat" }, h("b", {}, fa(n || 0)), h("span", {}, `${l} · ${fa(t ? Math.round(((n || 0) / t) * 100) : 0)}٪`)),
        ),
      ),
      h("div", { class: "grid-2" }, breakdown("به تفکیک استان", prov, t, { ic: "map" }), breakdown("به تفکیک شهرستان", cities, t, { ic: "map" })),
      h("div", { class: "grid-2" }, breakdown("به تفکیک حوزه کاری", cats, t, { ic: "tag" }), breakdown("به تفکیک کلمه کلیدی", kws, t, { ic: "search" })),
      h("div", { class: "grid-2" }, breakdown("وضعیت پیگیری", stats, t, { ic: "check" }), breakdown("پوشش راه‌های ارتباطی", chans, t, { ic: "phone", limit: 12 })),
    ),
  );
}
