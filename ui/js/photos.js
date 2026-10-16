// Photos: bundled, licence-credited pictures (img/*.webp) for each province, business category and
// product category, plus a live photo of any city from its Persian Wikipedia article (fetched by
// the app, cached on disk in HamistaSeller-Data/cache/photos; when offline a bundled photo always shows).
import { h, invoke } from "./core.js";
import { CREDITS } from "./photo-credits.js";
import { PROVINCES } from "./data.js";

const CENTRE = Object.fromEntries(Object.entries(PROVINCES).map(([p, cities]) => [p, cities[0]]));

const PROV = {
  "آذربایجان شرقی": "p-tabriz",
  "آذربایجان غربی": "p-urmia",
  "اردبیل": "p-ardabil",
  "اصفهان": "p-isfahan",
  "البرز": "p-karaj",
  "ایلام": "p-ilam",
  "بوشهر": "p-bushehr",
  "تهران": "p-tehran",
  "چهارمحال و بختیاری": "p-shahrekord",
  "خراسان جنوبی": "p-birjand",
  "خراسان رضوی": "p-mashhad",
  "خراسان شمالی": "p-bojnurd",
  "خوزستان": "p-ahvaz",
  "زنجان": "p-zanjan",
  "سمنان": "p-semnan",
  "سیستان و بلوچستان": "p-zahedan",
  "فارس": "p-shiraz",
  "قزوین": "p-qazvin",
  "قم": "p-qom",
  "کردستان": "p-sanandaj",
  "کرمان": "p-kerman",
  "کرمانشاه": "p-kermanshah",
  "کهگیلویه و بویراحمد": "p-yasuj",
  "گلستان": "p-gorgan",
  "گیلان": "p-rasht",
  "لرستان": "p-khorramabad",
  "مازندران": "p-sari",
  "مرکزی": "p-arak",
  "هرمزگان": "p-bandarabbas",
  "همدان": "p-hamadan",
  "یزد": "p-yazd",
};

// Fallback bundled keys for the 3 provinces whose .webp files were not bundled in v3.1/v3.2,
// used automatically when offline or before Wikipedia caches their centre photo.
const PROV_FALLBACK = {
  "p-rasht": "p-sari",
  "p-sanandaj": "p-kermanshah",
  "p-arak": "p-qazvin",
};

// first word that matches wins, so «کافه رستوران» is a café
const KEYWORD = [
  ["کافه", "k-cafe"],
  ["قهوه", "k-cafe"],
  ["رستوران", "k-restaurant"],
  ["غذا", "k-restaurant"],
  ["فست", "k-restaurant"],
  ["دندان", "k-dentist"],
  ["کلینیک", "k-dentist"],
  ["آرایش", "k-salon"],
  ["سالن", "k-salon"],
  ["موبایل", "k-mobile"],
  ["گوشی", "k-mobile"],
  ["املاک", "k-realestate"],
  ["مشاور", "k-realestate"],
  ["باشگاه", "k-gym"],
  ["ورزش", "k-gym"],
  ["داروخانه", "k-pharmacy"],
  ["دارو", "k-pharmacy"],
  ["آموزشگاه", "k-school"],
  ["مدرسه", "k-school"],
  ["تعمیر", "k-garage"],
  ["خودرو", "k-garage"],
  ["مبل", "k-furniture"],
  ["ساختمان", "k-construction"],
  ["پیمانکار", "k-construction"],
];

const PRODUCT = [
  ["عسل", "m-honey"],
  ["زعفران", "m-saffron"],
  ["موبایل", "m-mobile"],
  ["گوشی", "m-mobile"],
  ["پوشاک", "m-clothing"],
  ["لباس", "m-clothing"],
  ["مبل", "m-furniture"],
  ["خشکبار", "m-nuts"],
  ["پسته", "m-nuts"],
  ["آجیل", "m-nuts"],
  ["لوازم خانگی", "m-appliance"],
  ["یخچال", "m-appliance"],
  ["قطعات", "m-carparts"],
  ["لوازم یدکی", "m-carparts"],
  ["کافه", "m-cafe"],
  ["قهوه", "m-cafe"],
  ["اسپرسو", "m-cafe"],
  ["چای", "m-tea"],
  ["برنج", "m-rice"],
  ["روغن", "m-oil"],
];

const match = (table, text) => (text ? table.find(([w]) => text.includes(w))?.[1] : null);

/** {src, credit, fallbackSrc} for a bundled photo key, or null. `card` → the 4:3 crop. */
export function photo(key, card = false) {
  if (!key) return null;
  const suffix = card ? "-c" : "";
  if (CREDITS[key]) {
    return { src: `img/${key}${suffix}.webp`, credit: CREDITS[key] };
  }
  const fb = PROV_FALLBACK[key];
  if (fb && CREDITS[fb]) {
    return {
      src: `img/${key}${suffix}.webp`,
      fallbackSrc: `img/${fb}${suffix}.webp`,
      credit: CREDITS[fb],
      needsLive: true,
    };
  }
  return null;
}

export const provincePhoto = (province) => photo(PROV[province]);
export const keywordPhoto = (kw, card = false) => photo(match(KEYWORD, kw), card);
export const productPhoto = (q, card = false) => photo(match(PRODUCT, q), card);

const live = new Map();

/** The searched city's own photo when it is not the province centre (or when the province has no bundled photo);
 *  otherwise / offline: the province's bundled photo (or its regional fallback). */
export async function cityPhoto(province, city) {
  const base = provincePhoto(province);
  const hasBundled = base && !base.needsLive;
  const c = (city || "").trim() || (hasBundled ? "" : CENTRE[province] || "");
  if (!c || (hasBundled && c === CENTRE[province])) return base;
  if (!live.has(c)) live.set(c, invoke("city_photo", { city: c }).catch(() => ""));
  const src = await live.get(c);
  if (src) return { src, credit: "Wikipedia" };
  return base || photo("p-isfahan");
}

/** Sets a photo as the element's background once it has loaded (no flash of an empty box).
 *  Automatically falls back to `fallbackSrc` if a local file is missing. */
export function setPhoto(el, p) {
  const credit = el.querySelector(".hp-credit");
  if (!p) {
    delete el.dataset.src;
    el.style.backgroundImage = "";
    if (credit) credit.textContent = "";
    return;
  }
  if (el.dataset.src === p.src) return;
  el.dataset.src = p.src;

  const apply = (finalUrl, creditText) => {
    if (el.dataset.src !== p.src) return;
    el.style.backgroundImage = `url("${finalUrl}")`;
    el.classList.remove("photo-fade");
    void el.offsetWidth;
    el.classList.add("photo-fade");
    if (credit) credit.textContent = creditText ? `© ${creditText}` : "";
  };

  const img = new Image();
  img.onload = () => apply(p.src, p.credit);
  img.onerror = () => {
    if (p.fallbackSrc && p.fallbackSrc !== p.src) {
      const fb = new Image();
      fb.onload = () => apply(p.fallbackSrc, p.credit);
      fb.src = p.fallbackSrc;
    }
  };
  img.src = p.src;
}

/** The kit's banner: a photo with a dark fade, a kicker, a title, a line of text and an optional action. */
export function hero({ kicker = "", title = "", sub = "", photo: p = null, action = null, small = false }) {
  const t = h("h2", {}, title);
  const s = h("p", {}, sub);
  const k = h("span", { class: "hp-k" }, kicker);
  const el = h(
    "div",
    { class: `hero-photo ${small ? "sm" : ""}`.trim() },
    h("div", { class: "hp-body" }, k, t, s),
    action ? h("div", { class: "hp-act" }, action) : null,
    h("span", { class: "hp-credit", "data-no-tr": "" }),
  );
  el.update = ({ kicker: k2, title: t2, sub: s2, photo: p2 } = {}) => {
    if (k2 !== undefined) k.textContent = k2;
    if (t2 !== undefined) t.textContent = t2;
    if (s2 !== undefined) s.textContent = s2;
    if (p2 !== undefined) setPhoto(el, p2);
  };
  if (p) setPhoto(el, p);
  return el;
}

/** Photo cards (kit «Featured Course»): [{title, tag, photo, onclick}]. */
export function photoCards(items) {
  return h(
    "div",
    { class: "pcards" },
    items.map((it) => {
      const b = h(
        "button",
        { type: "button", class: "pcard", onclick: it.onclick, title: it.hint || "" },
        h("span", { class: "pc-t" }, h("b", {}, it.title), it.tag ? h("span", {}, it.tag) : null),
      );
      if (it.photo) b.style.backgroundImage = `url("${it.photo.fallbackSrc || it.photo.src}")`;
      return b;
    }),
  );
}