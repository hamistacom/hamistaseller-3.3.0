import json, urllib.request, urllib.parse, sys, os, time
UA = {"User-Agent": "HamistaSeller-build/3.1 (hamista.ir)"}
def get(url, tries=2):
    for i in range(tries):
        try:
            return urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=40).read()
        except urllib.error.HTTPError as e:
            if e.code != 429: raise
            time.sleep(4)
    raise RuntimeError("429")
jobs = json.load(open(sys.argv[1])); os.makedirs("cand", exist_ok=True)
meta = json.load(open("cand.json")) if os.path.exists("cand.json") else {}
for key, q in jobs.items():
    if key in meta: continue
    try:
        u = "https://api.openverse.org/v1/images/?" + urllib.parse.urlencode({"q": q, "page_size": 8, "license_type": "commercial", **({} if os.environ.get("ANY") else {"aspect_ratio": "wide"}), "mature": "false"})
        res = json.loads(get(u))["results"]
        res = [r for r in res if (r.get("width") or 0) >= 900][:3]
        meta[key] = []
        for i, r in enumerate(res):
            try:
                try:
                    data = get(r["url"])
                except Exception:
                    data = get(f'https://api.openverse.org/v1/images/{r["id"]}/thumb/?full_size=true', 3)
                open(f"cand/{key}_{i}.jpg", "wb").write(data)
                meta[key].append({"i": i, "url": r["url"], "title": r["title"], "creator": r.get("creator"), "license": f'CC {r["license"].upper()} {r.get("license_version","")}', "landing": r["foreign_landing_url"], "attribution": r.get("attribution")})
            except Exception as e:
                print("img err", key, i, e)
        print("ok", key, len(meta[key]), flush=True)
        json.dump(meta, open("cand.json", "w"), ensure_ascii=False, indent=1)
        time.sleep(1)
    except Exception as e:
        print("ERR", key, e, flush=True)
