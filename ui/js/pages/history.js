// History: scans (with «ادامه»), product analyses, brand analyses, campaigns, activity feed.
import { h, icon, invoke, fa, clear, jdate, ago } from "../core.js";
import { runsTable } from "./market.js";
import { refreshLists, go } from "../state.js";
import { runJob } from "../jobs.js";
import { historyTab } from "./outreach.js";
import { RUN_STATUS } from "./dashboard.js";

export async function render(root, params) {
  const tabs = [
    ["runs", "استخراج‌ها"],
    ["market", "تحلیل محصول"],
    ["brands", "تحلیل برند"],
    ["campaigns", "کمپین‌ها"],
    ["activity", "رویدادها"],
  ];
  let tab = params.tab || "runs";
  const bar = h("div", { class: "tabs" });
  const body = h("div", { class: "stack" });
  let seq = 0;
  const draw = async () => {
    const my = ++seq;
    clear(bar, tabs.map(([k, l]) => h("button", { class: k === tab ? "on" : "", type: "button", onclick: () => { tab = k; draw(); } }, l)));
    const box = h("div", { class: "stack" });
    if (tab === "runs") await runs(box);
    else if (tab === "market") await marketRuns(box);
    else if (tab === "brands") await brandRuns(box);
    else if (tab === "campaigns") await historyTab(box);
    else await activity(box);
    if (my === seq) clear(body, box);
  };
  root.append(h("div", { class: "stack" }, bar, body));
  await draw();
}

async function runs(root) {
  const list = await invoke("runs");
  if (!list.length) {
    root.append(h("div", { class: "card" }, h("div", { class: "empty" }, icon("scan", "lg"), "هنوز استخراجی انجام نشده است.", h("button", { class: "btn accent sm", type: "button", onclick: () => go("scan") }, "شروع استخراج"))));
    return;
  }
  root.append(
    h(
      "div",
      { class: "card flush" },
      h(
        "div",
        { class: "table-wrap" },
        h(
          "table",
          { class: "t" },
          h("thead", {}, h("tr", {}, ["زمان", "کلمه و منطقه", "بانک", "بررسی‌شده", "مخاطب جدید", "وضعیت", ""].map((t) => h("th", {}, t)))),
          h(
            "tbody",
            {},
            list.map((r) => {
              const [label, tone] = RUN_STATUS[r.status] || [r.status, ""];
              const canResume = r.resume && r.status !== "done" && r.status !== "running";
              return h(
                "tr",
                {},
                h("td", { class: "small" }, jdate(r.startedAt, true)),
                h("td", {}, h("div", { class: "name-cell" }, h("span", {}, r.keyword), h("span", { class: "c" }, [r.province, r.city].filter(Boolean).join(" — ")))),
                h("td", { class: "clip" }, r.listName),
                h("td", {}, `${fa(r.checked)} از ${fa(r.requested)}`),
                h("td", {}, fa(r.found)),
                h("td", { title: r.error }, h("span", { class: `pill ${tone}` }, label)),
                h("td", {}, canResume ? h("button", { class: "btn sm", type: "button", onclick: async () => { go("scan"); await runJob("scan", "ادامه استخراج", "scan_resume", { runId: r.id }); refreshLists(); } }, icon("play", "sm"), "ادامه") : null),
              );
            }),
          ),
        ),
      ),
    ),
  );
}

async function marketRuns(root) {
  const rows = await invoke("market_runs");
  if (!rows.length) {
    root.append(h("div", { class: "card" }, h("div", { class: "empty" }, icon("trend", "lg"), "هنوز تحلیل محصولی انجام نشده است.", h("button", { class: "btn accent sm", type: "button", onclick: () => go("market") }, "تحلیل محصول"))));
    return;
  }
  root.append(runsTable(rows, () => { clear(root); marketRuns(root); }));
}

async function brandRuns(root) {
  const rows = await invoke("brand_runs");
  if (!rows.length) {
    root.append(h("div", { class: "card" }, h("div", { class: "empty" }, icon("award", "lg"), "هنوز تحلیل برندی انجام نشده است.", h("button", { class: "btn accent sm", type: "button", onclick: () => go("brands") }, "تحلیل برند"))));
    return;
  }
  // score change against the brand's previous analysis
  const prev = {};
  const delta = rows
    .slice()
    .reverse()
    .map(([id, bid, , score]) => {
      const d = prev[bid] != null && score != null ? score - prev[bid] : null;
      if (score != null) prev[bid] = score;
      return [id, d];
    });
  const dmap = Object.fromEntries(delta);
  root.append(
    h(
      "div",
      { class: "card flush" },
      h(
        "div",
        { class: "table-wrap" },
        h(
          "table",
          { class: "t" },
          h("thead", {}, h("tr", {}, ["زمان", "برند", "امتیاز سلامت برند", "تغییر نسبت به تحلیل قبلی"].map((t) => h("th", {}, t)))),
          h(
            "tbody",
            {},
            rows.map(([id, bid, name, score, at]) =>
              h(
                "tr",
                { class: "click", onclick: () => go("brands", { id: bid }) },
                h("td", { class: "small" }, jdate(at, true)),
                h("td", {}, h("b", {}, name)),
                h("td", {}, score != null ? `${fa(score)} از ۱۰۰` : "—"),
                h("td", {}, dmap[id] ? h("span", { class: `trend ${dmap[id] > 0 ? "up" : "down"}` }, `${dmap[id] > 0 ? "+" : "−"}${fa(Math.abs(dmap[id]))}`) : h("span", { class: "x" }, "—")),
              ),
            ),
          ),
        ),
      ),
    ),
  );
}

async function activity(root) {
  const list = await invoke("activity", { limit: 200 });
  root.append(
    h(
      "div",
      { class: "card" },
      list.length ? h("div", { class: "feed" }, list.map((a) => h("div", { class: "it" }, h("div", { class: "grow" }, h("b", { class: "fw-400" }, a.text), h("span", {}, `${jdate(a.at, true)} · ${ago(a.at)}`))))) : h("div", { class: "empty" }, "رویدادی ثبت نشده است."),
    ),
  );
}
