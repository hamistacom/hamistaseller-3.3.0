// «بانک جامع» (online accounts whose plan includes it): search the shared bank of public business
// listings and copy chosen rows into one of your own banks.
import { h, icon, invoke, fa, clear, safe, select, input, toast } from "../core.js";
import { app, bankOptions, refreshLists } from "../state.js";

const PAGE = 50;

export async function render(root) {
  const q = input({ class: "input search-in", placeholder: "نام یا حوزه کاری…" });
  const province = input({ placeholder: "استان" });
  const city = input({ placeholder: "شهر" });
  const category = input({ placeholder: "حوزه کاری" });
  const bank = select(bankOptions(), app.lists[0]?.id);
  const body = h("div");
  const pager = h("div", { class: "pager" });
  const picked = new Map(); // id → row
  let offset = 0;
  let rows = [];
  let total = 0;

  const addBtn = h("button", { class: "btn primary", type: "button" }, icon("plus", "sm"), "افزودن به بانک");
  addBtn.addEventListener("click", async () => {
    const chosen = [...picked.values()];
    if (!chosen.length) return toast("ابتدا ردیف‌ها را انتخاب کنید.", "bad");
    addBtn.disabled = true;
    const r = await safe(invoke("mega_import", { listId: Number(bank.value), rows: chosen }));
    addBtn.disabled = false;
    if (!r) return;
    toast(`${fa(r.places)} کسب‌وکار جدید با ${fa(r.contacts)} راه ارتباطی اضافه شد${r.places < chosen.length ? " (تکراری‌ها رد شدند)" : ""}.`, "ok");
    picked.clear();
    await refreshLists();
    draw();
  });

  const draw = () => {
    addBtn.textContent = "";
    addBtn.append(icon("plus", "sm"), picked.size ? `افزودن ${fa(picked.size)} مورد به بانک` : "افزودن به بانک");
    if (!rows.length) {
      clear(body, h("div", { class: "empty" }, icon("database", "lg"), total === 0 && offset === 0 ? "نتیجه‌ای پیدا نشد. فیلترها را تغییر دهید." : ""));
      clear(pager);
      return;
    }
    const all = h("input", { type: "checkbox", checked: rows.every((r) => picked.has(r.id)), "aria-label": "انتخاب همه" });
    all.addEventListener("change", () => {
      for (const r of rows) all.checked ? picked.set(r.id, r) : picked.delete(r.id);
      draw();
    });
    clear(
      body,
      h(
        "div",
        { class: "table-wrap" },
        h(
          "table",
          { class: "t" },
          h("colgroup", {}, h("col", { class: "w-check" }), h("col"), h("col", { class: "c-phone" }), h("col"), h("col", { class: "c-status" })),
          h("thead", {}, h("tr", {}, h("th", {}, all), ["کسب‌وکار", "شماره‌ها", "منطقه", "اعتبار"].map((t) => h("th", {}, t)))),
          h(
            "tbody",
            {},
            rows.map((r) => {
              const cb = h("input", { type: "checkbox", checked: picked.has(r.id), "aria-label": "انتخاب" });
              cb.addEventListener("change", () => {
                cb.checked ? picked.set(r.id, r) : picked.delete(r.id);
                draw();
              });
              const nums = [...r.mobiles.split(","), ...r.landlines.split(",")].filter(Boolean);
              return h(
                "tr",
                { class: picked.has(r.id) ? "sel" : "" },
                h("td", {}, cb),
                h("td", {}, h("div", { class: "name-cell" }, h("span", { class: "n" }, r.name || "—"), h("span", { class: "c" }, [r.category, r.website, r.emails].filter(Boolean).join(" · ")))),
                h("td", {}, h("div", { class: "phones ltr" }, nums.slice(0, 2).map((n) => h("span", {}, n)), nums.length > 2 ? h("span", { class: "x" }, `+${fa(nums.length - 2)}`) : null)),
                h("td", { class: "clip" }, [r.province, r.city].filter(Boolean).join("، ") || "—"),
                h("td", {}, h("span", { class: "pill info", title: "تعداد دفعاتی که این کسب‌وکار گزارش شده" }, `${fa(r.reportCount)} گزارش`)),
              );
            }),
          ),
        ),
      ),
    );
    clear(
      pager,
      h("span", {}, `${fa(offset + 1)}–${fa(offset + rows.length)} از ${fa(total)}`),
      h(
        "div",
        { class: "row" },
        h("button", { class: "btn sm", type: "button", disabled: offset === 0, onclick: () => load(offset - PAGE) }, "قبلی"),
        h("button", { class: "btn sm", type: "button", disabled: offset + PAGE >= total, onclick: () => load(offset + PAGE) }, "بعدی"),
      ),
    );
  };

  let seq = 0;
  const load = async (off = 0) => {
    const my = ++seq;
    offset = Math.max(0, off);
    const r = await safe(
      invoke("mega_search", { search: { q: q.value.trim(), province: province.value.trim(), city: city.value.trim(), category: category.value.trim(), offset, limit: PAGE } }),
    );
    if (my !== seq || !r) return;
    rows = r.rows || [];
    total = r.total || 0;
    draw();
  };
  for (const i of [q, province, city, category]) i.addEventListener("keydown", (e) => e.key === "Enter" && load(0));

  root.append(
    h(
      "div",
      { class: "stack" },
      h("div", { class: "callout info" }, icon("info"), h("div", { class: "small" }, "بانک جامع از اطلاعات عمومی کسب‌وکارها ساخته شده و مدام کامل‌تر می‌شود. ردیف‌هایی را که لازم دارید انتخاب و به بانک خودتان اضافه کنید؛ تکراری‌ها خودکار رد می‌شوند. ارسال پیام فقط از راه‌های رسمی و با احترام به درخواست لغو مجاز است.")),
      h("div", { class: "card" }, h("div", { class: "filters" }, q, province, city, category, h("button", { class: "btn", type: "button", onclick: () => load(0) }, icon("search", "sm"), "جستجو"))),
      h("div", { class: "card flush" }, body, pager),
      h("div", { class: "bulk" }, h("span", { class: "grow" }, "بانک مقصد:"), bank, addBtn),
    ),
  );
  await load(0);
}
