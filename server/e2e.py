#!/usr/bin/env python3
# End-to-end test: launches the server as a child, drives the full API over HTTP, verifies, shuts down.
import os, json, time, base64, subprocess, signal, shutil, sys, urllib.request, urllib.error

DATA = "/tmp/hc-data"
PORT = 8891
B = f"http://127.0.0.1:{PORT}"
shutil.rmtree(DATA, ignore_errors=True)
env = dict(os.environ, HAMISTA_DATA_DIR=DATA, HAMISTA_BIND=f"127.0.0.1:{PORT}",
           HAMISTA_ADMIN_USER="admin", HAMISTA_ADMIN_PASS="admin123", HAMISTA_TOKEN_TTL_HOURS="72",
           RUST_LOG="warn")
srv = subprocess.Popen(["./target/debug/hamista-cloud"], env=env,
                       stdout=open("/tmp/hc.log", "w"), stderr=subprocess.STDOUT)
fails = []
try:
    for _ in range(60):
        try:
            urllib.request.urlopen(B + "/health", timeout=1); break
        except Exception:
            if srv.poll() is not None:
                print("server exited early:\n", open("/tmp/hc.log").read()); sys.exit(1)
            time.sleep(0.25)
    PUB = next(l.split()[1] for l in open(f"{DATA}/signing-key.txt") if "public_key" in l)

    def call(path, data=None, token=None, admin=None, method=None, raw=False):
        body = json.dumps(data).encode() if data is not None else None
        r = urllib.request.Request(B + path, data=body, method=method or ("POST" if data is not None else "GET"))
        if data is not None: r.add_header("Content-Type", "application/json")
        if token: r.add_header("Authorization", "Bearer " + token)
        if admin: r.add_header("x-admin-token", admin)
        try:
            resp = urllib.request.urlopen(r); return resp.status, (resp.read().decode() if raw else json.loads(resp.read()))
        except urllib.error.HTTPError as e:
            t = e.read().decode(); return e.code, (t if raw else json.loads(t))

    def ok(name, cond, extra=""):
        print(("PASS " if cond else "FAIL "), name, "" if cond else f"  >>> {extra}")
        if not cond: fails.append(name)

    c, r = call("/api/v1/admin/login", {"username": "admin", "password": "admin123"}); AT = r.get("token")
    ok("admin login", c == 200 and AT, r)
    ok("admin bad pass 401", call("/api/v1/admin/login", {"username": "admin", "password": "x"})[0] == 401)

    c, r = call("/api/v1/admin/plans", {"name": "طلایی", "maxDevices": 2, "scanQuota": 5, "sources": "maps,neshan", "megabank": True}, admin=AT)
    PLAN = r.get("id"); ok("plan create", c == 200 and PLAN, r)

    c, r = call("/api/v1/admin/users", {"username": "cafe1", "password": "secret1", "customerName": "کافه لمیز", "planId": PLAN, "expires": "2030-01-01"}, admin=AT)
    UID = r.get("id"); ok("user create", c == 200 and UID, r)
    ok("dup username 400", call("/api/v1/admin/users", {"username": "cafe1", "password": "xxxxxx"}, admin=AT)[0] == 400)

    c, r = call("/api/v1/auth/login", {"username": "cafe1", "password": "secret1", "machineId": "PC-AAA", "machineName": "laptop", "appVersion": "1.0.0"})
    TOK = r.get("token"); MACHINE = r.get("machine")
    ok("client login", c == 200 and TOK, r)
    ok("publicKey matches signing key", r.get("publicKey") == PUB, (r.get("publicKey"), PUB))
    ok("license megabank+quota", r["license"]["megabank"] and r["license"]["scanLeft"] == 5, r.get("license"))
    claims = json.loads(base64.urlsafe_b64decode(TOK.split(".")[1] + "==="))
    ok("token bound to user+machine", claims["user"] == "cafe1" and claims["machine"] == MACHINE, claims)
    ok("token ttl ~72h", 0 < claims["exp"] - claims["iat"] <= 72 * 3600 + 5, claims)
    ok("client bad pass 401", call("/api/v1/auth/login", {"username": "cafe1", "password": "no", "machineId": "PC-AAA"})[0] == 401)

    ok("2nd device ok", call("/api/v1/auth/login", {"username": "cafe1", "password": "secret1", "machineId": "PC-BBB"})[0] == 200)
    ok("3rd device blocked", call("/api/v1/auth/login", {"username": "cafe1", "password": "secret1", "machineId": "PC-CCC"})[0] == 403)

    c, r = call("/api/v1/license/heartbeat", {"appVersion": "1.0.1"}, token=TOK)
    ok("heartbeat refreshes token", c == 200 and r.get("token") and r["token"] != TOK, r)

    for _ in range(5): c, r = call("/api/v1/quota/consume", {"kind": "scan", "n": 1}, token=TOK)
    ok("quota 5 used -> left 0", c == 200 and r.get("scanLeft") == 0, r)
    ok("quota exceeded 429", call("/api/v1/quota/consume", {"kind": "scan", "n": 1}, token=TOK)[0] == 429)

    places = [{"placeKey": "0x1:0xA", "name": "کافه لمیز", "mobiles": ["09120000001"], "city": "تهران", "province": "تهران", "category": "کافه", "source": "maps", "website": "https://lamiz.ir"},
              {"placeKey": "0x2:0xB", "name": "رستوران شب", "mobiles": ["09120000002"], "city": "اصفهان", "province": "اصفهان", "category": "رستوران", "source": "maps"}]
    c, r = call("/api/v1/megabank/ingest", {"places": places}, token=TOK); ok("ingest 2 new", c == 200 and r["added"] == 2, r)
    c, r = call("/api/v1/megabank/ingest", {"places": [{"placeKey": "0x1:0xA", "mobiles": ["09120000009"]}]}, token=TOK)
    ok("ingest merge (not new)", c == 200 and r["merged"] == 1 and r["added"] == 0, r)
    c, r = call("/api/v1/megabank/search", {"province": "تهران"}, token=TOK)
    ok("client mega search + csv union", c == 200 and r["total"] == 1 and "09120000009" in r["rows"][0]["mobiles"], r)

    c, r = call("/api/v1/admin/megabank/search", {}, admin=AT); ok("admin search all=2", c == 200 and r["total"] == 2, r)
    c, r = call("/api/v1/admin/megabank/facets", {}, admin=AT); ok("facets", dict(r["provinces"]).get("تهران") == 1, r)
    c, r = call("/api/v1/admin/megabank/export", {}, admin=AT, raw=True); ok("csv export", c == 200 and "کافه لمیز" in r and r.startswith("﻿"), r[:40])

    call(f"/api/v1/admin/users/{UID}", {"status": "suspended"}, admin=AT, method="PUT")
    ok("suspended client blocked 403", call("/api/v1/license/heartbeat", {}, token=TOK)[0] == 403)
    call(f"/api/v1/admin/users/{UID}", {"status": "active"}, admin=AT, method="PUT")

    call("/api/v1/admin/notifications", {"title": "خوش آمدید", "audience": "all"}, admin=AT)
    c, r = call("/api/v1/notifications", token=TOK); NID = r["items"][0]["id"]
    ok("notif unread", len(r["items"]) == 1 and not r["items"][0]["read"], r)
    call("/api/v1/notifications/read", {"id": NID}, token=TOK)
    ok("notif read", call("/api/v1/notifications", token=TOK)[1]["items"][0]["read"], "")

    call("/api/v1/admin/version", {"version": "1.2.0", "notes": "n"}, admin=AT, method="PUT")
    ok("agent version public", call("/api/v1/agent/version")[1]["version"] == "1.2.0")
    call("/api/v1/admin/scenarios/maps", {"body": {"steps": ["open", "read"]}}, admin=AT, method="PUT")
    c, r = call("/api/v1/scenarios/maps"); ok("scenario fetch v1", r["version"] == 1 and r["body"]["steps"] == ["open", "read"], r)
    ok("scenario version bump", call("/api/v1/admin/scenarios/maps", {"body": {"steps": ["x"]}}, admin=AT, method="PUT")[1]["version"] == 2)

    ok("login logs", len(call("/api/v1/admin/logs?kind=login", admin=AT)[1]["items"]) >= 3)
    c, r = call("/api/v1/admin/overview", admin=AT)
    ok("overview", c == 200 and r["users"] == 1 and r["megabank"] == 2 and r["devices"] == 2, r)
    ok("no token 401", call("/api/v1/megabank/search", {})[0] == 401)
finally:
    srv.send_signal(signal.SIGINT);
    try: srv.wait(timeout=5)
    except Exception: srv.kill()

print("\n" + ("ALL PASS ✅" if not fails else "FAILURES: " + ", ".join(fails)))
sys.exit(1 if fails else 0)
