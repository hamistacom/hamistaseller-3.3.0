// English interface (the sidebar «EN / فا» button). Persian is the source language: in English mode
// every text node and label is looked up in i18n-en.json — exact strings, and strings with {0}…
// placeholders as patterns — and swapped as the page draws. Anything not in the dictionary
// (business names, addresses, user data) stays as it is.
let lang = "fa";
try {
  lang = localStorage.getItem("lang") === "en" ? "en" : "fa";
} catch {}
export const LANG = lang;
export const EN = lang === "en";

export function setLang(next) {
  try {
    localStorage.setItem("lang", next);
  } catch {}
  location.reload();
}

const exact = new Map();
const patterns = [];
const esc = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

if (EN) {
  document.documentElement.lang = "en";
  document.documentElement.dir = "ltr";
  const dict = await fetch("i18n-en.json").then((r) => r.json()).catch(() => ({}));
  for (const [fa, en] of Object.entries(dict)) {
    if (/\{\d+\}/.test(fa)) {
      if (fa.replace(/\{\d+\}/g, "").trim().length < 4) continue; // "{0} از {1}"-style: would rewrite user text
      const order = [...fa.matchAll(/\{(\d+)\}/g)].map((m) => Number(m[1]));
      const re = new RegExp(`^${esc(fa).replace(/\\\{\d+\\\}/g, "([\\s\\S]+?)")}$`);
      patterns.push({ re, en, order, lead: fa.split("{")[0], weight: fa.replace(/\{\d+\}/g, "").length });
    } else exact.set(fa, en);
  }
  // most specific first: more fixed text beats a loose "{0} از {1}"
  patterns.sort((a, b) => b.weight - a.weight);
}

const PERSIAN = /[؀-ۿ]/;

/** Persian → English for one string (whitespace around it is kept). */
export function tr(text) {
  if (!EN || !text || !PERSIAN.test(text)) return text;
  const core = text.trim();
  const pre = text.slice(0, text.indexOf(core));
  const post = text.slice(pre.length + core.length);
  const hit = exact.get(core);
  if (hit != null) return pre + hit + post;
  for (const p of patterns) {
    if (p.lead && !core.startsWith(p.lead)) continue;
    const m = core.match(p.re);
    if (!m) continue;
    let out = p.en;
    p.order.forEach((n, i) => {
      out = out.replace(`{${n}}`, tr(m[i + 1]));
    });
    return pre + out + post;
  }
  return text;
}

const ATTRS = ["placeholder", "title", "aria-label"];
// user data stays as typed: notes being edited, business names, contact people
const KEEP = "textarea, [data-no-tr], .cc-name, .cc-person, .name-cell, .ltr, .mono";
const kept = (el) => !!el?.closest?.(KEEP);

// labels on the element (only set on change: setting it again would re-trigger the observer)
function attrs(el) {
  for (const a of ATTRS) {
    const v = el.getAttribute(a);
    const t = v && PERSIAN.test(v) ? tr(v) : v;
    if (t !== v) el.setAttribute(a, t);
  }
}

function walk(root) {
  if (root.nodeType === Node.TEXT_NODE) {
    if (kept(root.parentElement)) return;
    const t = tr(root.nodeValue);
    if (t !== root.nodeValue) root.nodeValue = t;
    return;
  }
  if (root.nodeType !== Node.ELEMENT_NODE) return;
  attrs(root); // a kept element (e.g. a note textarea) still gets its placeholder translated
  if (kept(root)) return;
  const it = document.createTreeWalker(root, NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT, {
    acceptNode: (n) => {
      if (n.nodeType !== Node.ELEMENT_NODE) return NodeFilter.FILTER_ACCEPT;
      attrs(n);
      return n.matches(KEEP) ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_SKIP;
    },
  });
  for (let n = it.nextNode(); n; n = it.nextNode()) {
    if (PERSIAN.test(n.nodeValue)) {
      const t = tr(n.nodeValue);
      if (t !== n.nodeValue) n.nodeValue = t;
    }
  }
}

if (EN) {
  const start = () => {
    walk(document.body);
    new MutationObserver((list) => {
      for (const m of list) {
        if (m.type === "characterData") walk(m.target);
        else if (m.type === "attributes") walk(m.target);
        else m.addedNodes.forEach(walk);
      }
    }).observe(document.body, { childList: true, subtree: true, characterData: true, attributes: true, attributeFilter: ATTRS });
    document.title = tr(document.title);
  };
  if (document.body) start();
  else document.addEventListener("DOMContentLoaded", start);
}
