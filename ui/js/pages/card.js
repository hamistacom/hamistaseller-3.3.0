// Contact card: full details (nothing truncated), quick actions, status & note, edit / add.
import { h, icon, invoke, modal, safe, copy, field, input, select, toast, confirmBox, jdate, fa, handle, initial, appBadges, KIND, KINDS, STATUS } from "../core.js";
import { app, go, listName, bankOptions, refreshLists } from "../state.js";

const LOG = { call: ["تماس", "phone"], message: ["پیام", "chat"], meeting: ["جلسه", "users"], deal: ["معامله", "award"], note: ["یادداشت", "edit"] };
const pad2 = (n) => String(n).padStart(2, "0");
/** Date → "YYYY-MM-DD HH:MM" (local), the form the backend keeps. */
const stamp = (d) => `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())} ${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
const toLocalInput = (s) => (s ? s.replace(" ", "T") : "");
const fromLocalInput = (s) => (s ? s.replace("T", " ").slice(0, 16) : "");

/** B2B follow-up: the next reminder and a timeline of calls, messages, meetings and deals. */
function crmSection(p, onChange) {
  const timeline = h("div", { class: "crm-list" });
  const next = input({ type: "datetime-local", dir: "ltr", value: toLocalInput(p.followUpAt) });
  const kind = select(Object.entries(LOG).map(([k, v]) => [k, v[0]]), "call");
  const note = h("textarea", { class: "input", rows: 2, placeholder: "چه گفتید و چه قولی گرفتید؟" });
  const nextLog = input({ type: "datetime-local", dir: "ltr" });
  const load = async () => {
    const logs = (await safe(invoke("place_logs", { id: p.id }))) || [];
    timeline.replaceChildren(
      ...(logs.length
        ? logs.map((l) => {
            const [label, ic] = LOG[l.kind] || LOG.note;
            return h(
              "div",
              { class: `crm-item k-${l.kind}` },
              h("span", { class: "crm-k" }, icon(ic, "sm"), label),
              h("div", { class: "grow stack tight" }, h("span", { class: "crm-note" }, l.note), h("span", { class: "small muted" }, jdate(l.createdAt, true))),
              h("button", { class: "btn ghost sm icon-only", type: "button", "aria-label": "حذف", onclick: async () => { if ((await safe(invoke("place_log_delete", { id: l.id }))) !== undefined) load(); } }, icon("trash", "sm")),
            );
          })
        : [h("div", { class: "small muted" }, "هنوز تعاملی ثبت نشده است.")]),
    );
  };
  const setNext = async (value, msg) => {
    if ((await safe(invoke("follow_up_set", { id: p.id, at: value }), msg)) === undefined) return;
    p.followUpAt = value;
    next.value = toLocalInput(value);
    onChange?.();
  };
  const soon = (days) => () => {
    const d = new Date();
    d.setDate(d.getDate() + days);
    d.setHours(10, 0, 0, 0);
    setNext(stamp(d), "پیگیری زمان‌بندی شد");
  };
  const add = async () => {
    const r = await safe(invoke("place_log_add", { id: p.id, kind: kind.value, note: note.value, next: nextLog.value ? fromLocalInput(nextLog.value) : null }), "در تایم‌لاین ثبت شد");
    if (r === undefined) return;
    note.value = "";
    if (nextLog.value) {
      p.followUpAt = fromLocalInput(nextLog.value);
      next.value = nextLog.value;
      nextLog.value = "";
    }
    load();
    onChange?.();
  };
  load();
  return h(
    "div",
    { class: "stack crm" },
    h("span", { class: "label-t" }, "پیگیری و تایم‌لاین مذاکره"),
    h(
      "div",
      { class: "row wrap crm-next" },
      field("پیگیری بعدی", next),
      h("button", { class: "btn sm", type: "button", onclick: () => setNext(next.value ? fromLocalInput(next.value) : "", next.value ? "پیگیری زمان‌بندی شد" : "پیگیری حذف شد") }, icon("clock", "sm"), "ثبت زمان"),
      h("button", { class: "btn ghost sm", type: "button", onclick: soon(1) }, "فردا"),
      h("button", { class: "btn ghost sm", type: "button", onclick: soon(3) }, "۳ روز بعد"),
      h("button", { class: "btn ghost sm", type: "button", onclick: soon(7) }, "هفته بعد"),
      p.followUpAt ? h("button", { class: "btn ghost sm", type: "button", onclick: () => setNext("", "پیگیری حذف شد") }, icon("x", "sm"), "حذف زمان") : null,
    ),
    h("div", { class: "crm-form" }, field("نوع", kind), field("شرح تعامل", note), field("پیگیری بعدی (اختیاری)", nextLog), h("button", { class: "btn sm primary", type: "button", onclick: add }, icon("plus", "sm"), "ثبت")),
    timeline,
  );
}

const SOCIAL = ["instagram", "telegram", "whatsapp", "eitaa", "bale"];

function contactRow(c, place) {
  const k = c.kind === "website" ? { label: "وب‌سایت", icon: "globe" } : KIND[c.kind] || { label: c.kind, icon: "info" };
  const isLink = c.value.startsWith("http");
  const acts = [];
  if (c.kind === "mobile") {
    acts.push(
      h(
        "button",
        {
          class: "btn ghost sm icon-only k-whatsapp",
          type: "button",
          title: "گفتگو در واتساپ (دستیار)",
          onclick: () => safe(invoke("assist_open", { messenger: "whatsapp", to: c.value, text: "" })),
        },
        icon("whatsapp", "sm"),
      ),
      h(
        "button",
        {
          class: "btn ghost sm icon-only k-telegram",
          type: "button",
          title: "گفتگو در تلگرام (دستیار)",
          onclick: () => safe(invoke("assist_open", { messenger: "telegram", to: c.value, text: "" })),
        },
        icon("telegram", "sm"),
      ),
    );
  }
  if ((c.kind === "mobile" || c.kind === "landline") && place?.mapsUrl) {
    acts.push(
      h(
        "button",
        {
          class: "btn ghost sm icon-only",
          type: "button",
          title: place.source === "divar" || place.source === "sheypoor" ? "باز کردن لینک آگهی" : "باز کردن لینک منبع / نقشه",
          onclick: () => safe(invoke("open_url", { url: place.mapsUrl })),
        },
        icon("external", "sm"),
      ),
    );
  }
  if (isLink) {
    acts.push(
      h("button", { class: "btn ghost sm icon-only", type: "button", title: "باز کردن", onclick: () => safe(invoke("open_url", { url: c.value })) }, icon("external", "sm")),
    );
  }
  acts.push(h("button", { class: "btn ghost sm icon-only", type: "button", title: "کپی", onclick: () => copy(c.value) }, icon("copy", "sm")));
  const delivery = { sent: ["پیامک ارسال شد", "ok"], failed: ["پیامک ناموفق", "bad"], invalid: ["نامعتبر", "bad"] }[c.delivery];
  return h(
    "div",
    { class: "citem" },
    h("span", { class: `k k-${c.kind}` }, icon(k.icon, "sm"), k.label),
    h(
      "span",
      { class: "v" },
      h("span", { class: isLink || c.kind === "email" || c.kind === "mobile" || c.kind === "landline" ? "ltr" : "", title: c.value }, isLink ? handle(c.value) || c.value : c.value),
      appBadges(c),
      c.label ? h("span", { class: "small muted ms-6" }, `(${c.label})`) : null,
    ),
    c.origin === "website" ? h("span", { class: "pill", title: "از سایت خود کسب‌وکار" }, "سایت") : null,
    c.origin === "divar" ? h("span", { class: "pill info", title: "استخراج‌شده از اطلاعات تماس دیوار" }, "دیوار") : null,
    c.origin === "sheypoor" ? h("span", { class: "pill info", title: "استخراج‌شده از اطلاعات تماس شیپور" }, "شیپور") : null,
    delivery ? h("span", { class: `pill ${delivery[1]}` }, delivery[0]) : null,
    h("span", { class: "acts" }, acts),
  );
}

/** Opens the card of place `id`. `onChange()` runs after edits / status / delete. */
export async function openCard(id, { onChange } = {}) {
  const p = await safe(invoke("place", { id }));
  if (!p) return;
  const isAd = p.source === "divar" || p.source === "sheypoor" || (p.mapsUrl && (p.mapsUrl.includes("divar.ir") || p.mapsUrl.includes("sheypoor.com")));
  const isPage = p.source === "telegram" || p.source === "instagram";
  const status = select(Object.entries(STATUS).map(([k, v]) => [k, v.label]), p.status, { class: "input" });
  const note = h("textarea", { class: "input", rows: 2, placeholder: "یادداشت پیگیری (مثلاً: قرار تماس دوشنبه)" }, p.note);
  const contacts = [...p.contacts, ...(p.website ? [{ id: 0, kind: "website", value: p.website }] : [])];
  const rows = [
    ["آدرس / محدوده", p.address],
    [
      isAd ? "لینک آگهی" : isPage ? "لینک کانال / پیج" : "لینک نقشه",
      p.mapsUrl
        ? h(
            "button",
            { class: "link small ltr", type: "button", onclick: () => safe(invoke("open_url", { url: p.mapsUrl })) },
            icon("external", "sm"),
            isAd ? "مشاهده صفحه آگهی" : isPage ? "باز کردن کانال / پیج" : "مشاهده روی نقشه",
          )
        : "",
    ],
    ["حوزه کاری", p.category],
    ["استان / شهرستان", [p.province, p.city].filter(Boolean).join("، ")],
    ["امتیاز", p.rating ? `${fa(p.rating)} از ۵ · ${fa(p.reviews || 0)} نظر` : ""],
    ["کلمه کلیدی", p.keyword],
    ["برچسب‌ها", p.tags ? h("span", { class: "chips" }, p.tags.split(",").map((t) => h("span", { class: "chip" }, t))) : ""],
    ["بانک", listName(p.listId)],
    ["ثبت", `${jdate(p.createdAt, true)}${p.source === "import" ? " · از فایل" : p.source === "manual" ? " · دستی" : p.source === "divar" ? " · دیوار" : p.source === "sheypoor" ? " · شیپور" : ""}`],
  ].filter(([, v]) => v);

  const pic = h("div", { class: "avatar cc-pic" }, initial(p.name));
  if (p.photo) {
    invoke("place_photo", { id: p.id })
      .then((src) => src && pic.replaceChildren(h("img", { src, alt: "" })))
      .catch(() => {});
  }
  const body = [
    h(
      "div",
      { class: "cc-head" },
      pic,
      h(
        "div",
        { class: "grow stack tight" },
        h("div", { class: "cc-name" }, p.name),
        h(
          "div",
          { class: "cc-sub" },
          [p.category, p.city || p.province, p.address].filter(Boolean).join(" · "),
          p.mapsUrl
            ? h(
                "button",
                { class: "link small", type: "button", onclick: () => safe(invoke("open_url", { url: p.mapsUrl })) },
                icon(isAd || isPage ? "external" : "map", "sm"),
                isAd ? "مشاهده آگهی" : isPage ? "کانال / پیج" : "روی نقشه",
              )
            : null,
        ),
        p.contactPerson ? h("div", { class: "cc-person" }, icon("user", "sm"), p.contactPerson, p.contactRole ? h("span", { class: "muted" }, ` — ${p.contactRole}`) : null) : null,
      ),
    ),
    h(
      "div",
      { class: "cc-grid" },
      h(
        "div",
        { class: "stack tight" },
        h("span", { class: "label-t" }, `راه‌های ارتباطی (${fa(contacts.length)})`),
        h("div", { class: "clist" }, contacts.length ? contacts.map((c) => contactRow(c, p)) : h("div", { class: "citem muted" }, "راه ارتباطی ثبت نشده")),
      ),
      h(
        "div",
        { class: "stack tight" },
        h("span", { class: "label-t" }, "مشخصات و لینک"),
        h("dl", { class: "kv compact" }, rows.map(([k, v]) => [h("dt", {}, k), h("dd", {}, v)])),
        field("وضعیت پیگیری", status),
        note,
      ),
    ),
    crmSection(p, onChange),
  ];

  const m = modal({
    title: "کارت مشتری",
    wide: true,
    body,
    left: h(
      "button",
      {
        class: "btn danger sm",
        type: "button",
        onclick: async () => {
          if (!(await confirmBox(`«${p.name}» حذف شود؟`, { ok: "حذف", danger: true }))) return;
          if (await safe(invoke("places_delete", { ids: [p.id] }), "حذف شد")) {
            m.close();
            await refreshLists();
            onChange?.();
          }
        },
      },
      icon("trash", "sm"),
      "حذف",
    ),
    actions: [
      { label: "ویرایش", icon: "edit", onClick: () => { m.close(); openEditor(p, { onChange }); } },
      {
        label: "ذخیره",
        kind: "primary",
        onClick: async () => {
          const ok = await safe(invoke("place_status", { id: p.id, status: status.value, note: note.value }), "ذخیره شد");
          if (ok === undefined) return false;
          onChange?.();
        },
      },
    ],
  });
}

/** Add (place.id = 0) or edit a contact. */
export function openEditor(place = {}, { onChange, listId } = {}) {
  const p = { id: 0, listId: listId || app.lists[0]?.id, status: "new", contacts: [], ...place };
  const f = {
    name: input({ value: p.name || "", placeholder: "مثلاً کافه لمیز" }),
    person: input({ value: p.contactPerson || "", placeholder: "مثلاً علی امینی" }),
    role: input({ value: p.contactRole || "", placeholder: "مدیرعامل، مدیر فروش…" }),
    category: input({ value: p.category || "" }),
    province: input({ value: p.province || "" }),
    city: input({ value: p.city || "" }),
    address: input({ value: p.address || "" }),
    website: input({ value: p.website || "", dir: "ltr", placeholder: "https://" }),
    tags: input({ value: p.tags || "", placeholder: "با ویرگول جدا کنید" }),
    bank: select(bankOptions(), p.listId),
  };
  const list = h("div", { class: "stack tight" });
  const addRow = (c = { kind: "mobile", value: "" }) => {
    const kind = select(KINDS.map((k) => [k, KIND[k].label]), c.kind);
    const val = input({ value: c.value, dir: "ltr" });
    const row = h("div", { class: "edit-row" }, kind, val, h("button", { class: "btn ghost sm icon-only", type: "button", "aria-label": "حذف", onclick: () => row.remove() }, icon("x", "sm")));
    row.read = () => ({ kind: kind.value, value: val.value.trim(), label: c.label || "", origin: c.origin || "manual", id: c.id || 0 });
    list.append(row);
    if (!c.value) val.focus();
  };
  (p.contacts.length ? p.contacts : [{ kind: "mobile", value: "" }]).forEach(addRow);
  const body = [
    h("div", { class: "grid-2" }, field("نام کسب‌وکار *", f.name), p.id ? field("بانک", h("div", { class: "input" }, listName(p.listId))) : field("بانک", f.bank)),
    h("div", { class: "grid-2" }, field("نام مسئول / فروشنده", f.person), field("سمت", f.role)),
    h(
      "div",
      { class: "stack tight" },
      h("span", { class: "label-t" }, "راه‌های ارتباطی"),
      list,
      h("button", { class: "btn sm ghost", type: "button", onclick: () => addRow() }, icon("plus", "sm"), "افزودن شماره یا شبکه"),
      h("span", { class: "hint small faint" }, "اینستاگرام، تلگرام، ایتا و بله را می‌توانید به شکل آیدی (@name) یا لینک وارد کنید."),
    ),
    h("div", { class: "grid-3" }, field("دسته", f.category), field("استان", f.province), field("شهر", f.city)),
    field("آدرس", f.address),
    h("div", { class: "grid-2" }, field("وب‌سایت", f.website), field("برچسب‌ها", f.tags)),
  ];
  modal({
    title: p.id ? "ویرایش مخاطب" : "افزودن مخاطب",
    wide: true,
    body,
    actions: [
      { label: "انصراف" },
      {
        label: "ذخیره",
        kind: "primary",
        onClick: async () => {
          const contacts = [...list.children].map((r) => r.read()).filter((c) => c.value);
          const place = {
            ...p,
            listId: p.id ? p.listId : Number(f.bank.value),
            name: f.name.value,
            contactPerson: f.person.value,
            contactRole: f.role.value,
            category: f.category.value,
            province: f.province.value,
            city: f.city.value,
            address: f.address.value,
            website: f.website.value.trim(),
            tags: f.tags.value,
            contacts,
          };
          const r = await safe(invoke("place_save", { place }));
          if (!r) return false;
          toast(r.duplicates.length ? `ذخیره شد. این موارد قبلاً برای مخاطب دیگری در همین بانک ثبت شده بود: ${r.duplicates.join("، ")}` : "ذخیره شد", "ok");
          await refreshLists();
          onChange?.(r.id);
        },
      },
    ],
  });
  f.name.focus();
}

export const goToContacts = (listId) => go("contacts", { list: listId });
export { SOCIAL };