//! Hybrid licensing end to end: the REAL hamista-cloud server (in-process, on a local port) driven by
//! the desktop's own `cloud`, `license` and `admin` modules — admin panel and customer copy together.
use harness::db::{Contact, Db, Place};
use harness::{admin, cloud, license};
use serde_json::json;
use std::net::SocketAddr;

const SEED: &str = "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20"; // = test-key.pub

fn tmp_db(name: &str) -> Db {
    let dir = std::env::temp_dir().join(format!("hs-cloud-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    Db::open(&dir.join("hamista.db")).unwrap()
}

async fn start_server() -> String {
    let dir = std::env::temp_dir().join(format!("hs-cloud-srv-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::env::set_var("HAMISTA_ADMIN_USER", "admin");
    std::env::set_var("HAMISTA_ADMIN_PASS", "admin123");
    std::env::set_var("HAMISTA_SIGNING_SEED", SEED); // the admin app's key, as the server guide says
    let cfg = hamista_cloud::state::Config { bind: String::new(), data_dir: dir, token_ttl_hours: 72, agent_min_version: "1.0.0".into() };
    let state = hamista_cloud::state::AppState::new(cfg).unwrap();
    hamista_cloud::app::seed_admin(&state);
    let app = hamista_cloud::app::build(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await.unwrap();
    });
    format!("http://{addr}")
}

fn place(list_id: i64, key: &str, name: &str, source: &str, mobile: &str) -> Place {
    Place {
        list_id,
        place_key: key.into(),
        name: name.into(),
        province: "تهران".into(),
        city: "تهران".into(),
        category: "کافه".into(),
        source: source.into(),
        note: "یادداشت خصوصی مشتری".into(),
        tags: "vip".into(),
        contacts: vec![
            Contact { kind: "mobile".into(), value: mobile.into(), origin: "maps".into(), ..Default::default() },
            // typed in by the user (e.g. the owner's personal number): must never be uploaded
            Contact { kind: "mobile".into(), value: format!("{mobile}9"), origin: "manual".into(), ..Default::default() },
        ],
        ..Default::default()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn hybrid_admin_and_customer_flow() {
    let url = start_server().await;

    // ---------- admin panel: own key, connect to the server ----------
    let adm = tmp_db("admin");
    let key = license::import_key(&adm, SEED).unwrap();
    assert!(key.embedded_matches, "test build must embed test-key.pub");
    assert!(cloud::admin_connect(&adm, &url, "admin", "wrong", &key.public_key).await.is_err());
    let conn = cloud::admin_connect(&adm, &url, "admin", "admin123", &key.public_key).await.unwrap();
    assert!(conn.connected);
    assert_eq!(conn.key_matches, Some(true), "server signs with the admin app's key");

    // the panel renews an expired / lost session by itself
    adm.kv_set("admin_token", &harness::logic::b64_encode(&harness::secrets::seal_str("stale").unwrap())).unwrap();
    let plan = cloud::admin_call(&adm, "POST", "/plans", Some(json!({ "name": "طلایی", "maxDevices": 1, "scanQuota": 2, "sources": "maps", "megabank": true })))
        .await
        .unwrap();
    let plan_id = plan["id"].as_i64().unwrap();
    let user = cloud::admin_call(&adm, "POST", "/users", Some(json!({ "username": "cafe", "password": "secret1", "customerName": "کافه لمیز", "planId": plan_id, "expires": "2099-01-01" })))
        .await
        .unwrap();
    let uid = user["id"].as_i64().unwrap();
    assert!(cloud::admin_call(&adm, "GET", "/../auth/login", None).await.is_err(), "proxy stays inside /admin");

    // ---------- customer copy: online account ----------
    let cust = tmp_db("customer");
    assert!(cloud::login(&cust, &url, "cafe", "bad-pass").await.is_err());
    let st = cloud::login(&cust, &url, "cafe", "secret1").await.unwrap();
    assert!(st.valid, "{}", st.message);
    assert_eq!((st.mode, st.megabank, st.scan_left), ("online", true, 2));
    assert_eq!(st.customer, "کافه لمیز");
    assert!(license::source_allowed(&st, "maps").is_ok());
    assert!(license::source_allowed(&st, "neshan").is_err(), "plan only allows maps");

    // quota: 2 per month, atomic on the server
    cloud::consume(&cust, "scan", 1).await.unwrap();
    cloud::consume(&cust, "scan", 1).await.unwrap();
    assert!(cloud::consume(&cust, "scan", 1).await.is_err(), "third scan is over the plan quota");
    let st = cloud::heartbeat(&cust).await.unwrap();
    assert_eq!(st.scan_left, 0);
    assert!(cloud::precheck(&cust, "scan").is_err(), "refused locally from the token too");

    // ---------- mega-bank: only public fields of engine-found places ----------
    let list = 1;
    cust.insert_place(&place(list, "0x1:0xA", "کافه لمیز", "maps", "09120000001")).unwrap();
    cust.insert_place(&place(list, "file:1", "مشتری شخصی", "import", "09120000002")).unwrap();
    let (batch, _) = cloud::public_batch(&cust, 0, 100).unwrap();
    assert_eq!(batch.len(), 1, "file imports never leave the PC");
    let body = serde_json::to_string(&batch).unwrap();
    assert!(!body.contains("یادداشت") && !body.contains("vip") && !body.contains("status"), "no private fields: {body}");
    assert!(!body.contains("091200000019"), "manual contacts stay on the PC: {body}");
    assert_eq!(cloud::mega_sync(&cust).await.unwrap(), 1);
    assert_eq!(cloud::mega_sync(&cust).await.unwrap(), 0, "nothing is sent twice");
    let mega = cloud::admin_call(&adm, "POST", "/megabank/search", Some(json!({}))).await.unwrap();
    assert_eq!(mega["total"], 1);
    assert_eq!(mega["rows"][0]["mobiles"], "09120000001");

    // customer side of the shared bank: search, add to a bank, and it is not re-uploaded
    let found = cloud::mega_search(&cust, &cloud::MegaSearch { province: "تهران".into(), limit: 50, ..Default::default() }).await.unwrap();
    let rows = found["rows"].as_array().unwrap().clone();
    let other = cust.list_create("بانک جامع", "").unwrap();
    assert_eq!(cloud::mega_import(&cust, other, &rows).unwrap(), (1, 1));
    assert_eq!(cloud::mega_import(&cust, other, &rows).unwrap(), (0, 0), "same row twice is ignored");
    assert_eq!(cloud::mega_sync(&cust).await.unwrap(), 0);

    // ---------- notifications, version, scenario ----------
    cloud::admin_call(&adm, "POST", "/notifications", Some(json!({ "title": "خوش آمدید", "audience": "all" }))).await.unwrap();
    let n = cloud::notifications(&cust).await.unwrap();
    let nid = n["items"][0]["id"].as_i64().unwrap();
    cloud::notification_read(&cust, nid).await.unwrap();
    assert_eq!(cloud::notifications(&cust).await.unwrap()["items"][0]["read"], true);
    cloud::admin_call(&adm, "PUT", "/version", Some(json!({ "version": "2.1.0", "notes": "x" }))).await.unwrap();
    assert_eq!(cloud::agent_version(&cust).await.unwrap()["version"], "2.1.0");
    assert_eq!(cloud::scenario_refresh(&cust, "maps").await.unwrap(), 0, "no scenario yet");
    cloud::admin_call(&adm, "PUT", "/scenarios/maps", Some(json!({ "body": { "listJs": "return {items:[]};" } }))).await.unwrap();
    assert_eq!(cloud::scenario_refresh(&cust, "maps").await.unwrap(), 1);
    assert!(cust.kv_get("scenario_maps").unwrap().contains("listJs"));

    // ---------- remote control: suspend → the next heartbeat ends the session ----------
    cloud::admin_call(&adm, "PUT", &format!("/users/{uid}"), Some(json!({ "status": "suspended" }))).await.unwrap();
    let st = cloud::heartbeat(&cust).await.unwrap();
    assert!(!st.valid);
    assert!(st.message.contains("غیرفعال"), "{}", st.message);
    assert!(license::ensure(&cust).is_err());
    cloud::admin_call(&adm, "PUT", &format!("/users/{uid}"), Some(json!({ "status": "active" }))).await.unwrap();
    assert!(cloud::login(&cust, &url, "cafe", "secret1").await.unwrap().valid);

    // device removed by the admin → refused at the next heartbeat
    let devs = cloud::admin_call(&adm, "GET", &format!("/users/{uid}/devices"), None).await.unwrap();
    let dev = devs["items"][0]["id"].as_i64().unwrap();
    cloud::admin_call(&adm, "POST", &format!("/devices/{dev}/revoke"), None).await.unwrap();
    assert!(!cloud::heartbeat(&cust).await.unwrap().valid);

    // ---------- forged / foreign tokens are rejected offline ----------
    let machine = license::machine();
    let good = {
        let c2 = tmp_db("c2");
        cloud::admin_call(&adm, "POST", "/users", Some(json!({ "username": "cafe2", "password": "secret2", "planId": plan_id, "expires": "2099-01-01" })))
            .await
            .unwrap();
        cloud::login(&c2, &url, "cafe2", "secret2").await.unwrap();
        c2.kv_get("cloud_token").unwrap()
    };
    assert!(license::verify_cloud(&good, &machine).is_ok());
    assert!(license::verify_cloud(&good, "AAAA-BBBB-CCCC-DDDD").is_err(), "bound to one PC");
    let mut parts: Vec<String> = good.split('.').map(String::from).collect();
    let mut claims: serde_json::Value = serde_json::from_slice(&harness::logic::b64_decode(&parts[1]).unwrap()).unwrap();
    claims["scan_left"] = json!(999_999);
    parts[1] = harness::logic::b64_encode(claims.to_string().as_bytes());
    assert!(license::verify_cloud(&parts.join("."), &machine).is_err(), "edited token fails the signature");

    // ---------- quota: overshoot is capped, offline use is counted later ----------
    let p2 = cloud::admin_call(&adm, "POST", "/plans", Some(json!({ "name": "پایه", "maxDevices": 1, "scanQuota": 10, "sendQuota": 10 }))).await.unwrap();
    cloud::admin_call(&adm, "POST", "/users", Some(json!({ "username": "cafe3", "password": "secret3", "planId": p2["id"], "expires": "2099-01-01" })))
        .await
        .unwrap();
    let q = tmp_db("quota");
    cloud::login(&q, &url, "cafe3", "secret3").await.unwrap();
    assert!(cloud::consume(&q, "send", 500).await.is_err(), "campaign of 500 over a cap of 10");
    assert!(cloud::precheck(&q, "send").is_err(), "the next campaign is refused at once");
    assert_eq!(cloud::heartbeat(&q).await.unwrap().send_left, 0, "server filled the quota to its cap");
    // offline: allowed while quota is left, settled at the next heartbeat
    q.kv_set("cloud_url", "http://127.0.0.1:1").unwrap();
    cloud::consume(&q, "scan", 3).await.unwrap();
    assert_eq!(q.kv_get("quota_pending_scan").as_deref(), Some("3"));
    q.kv_set("cloud_url", &url).unwrap();
    let st = cloud::heartbeat(&q).await.unwrap();
    assert_eq!(st.scan_left, 7, "offline use was counted");
    assert!(q.kv_get("quota_pending_scan").is_none());

    // admin can take a plan away (null) and searches with any text pass the proxy
    let u4 = cloud::admin_call(&adm, "POST", "/users", Some(json!({ "username": "cafe4", "password": "secret4", "planId": plan_id }))).await.unwrap();
    cloud::admin_call(&adm, "PUT", &format!("/users/{}", u4["id"]), Some(json!({ "planId": null }))).await.unwrap();
    let found = cloud::admin_call(&adm, "GET", "/users?q=cafe4", None).await.unwrap();
    assert!(found["items"][0]["planId"].is_null(), "{found}");
    assert!(cloud::admin_call(&adm, "GET", "/users?q=%D8%A7(%2F)", None).await.is_ok());

    // ---------- free trial: off by default, then once per PC ----------
    let tdb = tmp_db("trial");
    assert_eq!(cloud::trial_info(&url).await.unwrap()["enabled"], false);
    assert!(cloud::start_trial(&tdb, &url, "کافه تست", "09121234567").await.is_err());
    assert!(cloud::admin_call(&adm, "PUT", "/trial", Some(json!({ "enabled": true, "planId": plan_id, "days": 3 }))).await.is_err(), "no mega-bank for anonymous trials");
    assert!(cloud::admin_call(&adm, "PUT", "/trial", Some(json!({ "enabled": true, "planId": 9999, "days": 3 }))).await.is_err(), "plan must exist");
    cloud::admin_call(&adm, "PUT", "/trial", Some(json!({ "enabled": true, "planId": p2["id"], "days": 3 }))).await.unwrap();
    assert_eq!(cloud::trial_info(&url).await.unwrap()["days"], 3);
    assert!(cloud::start_trial(&tdb, &url, "ک", "123").await.is_err(), "name and mobile are checked");
    let st = cloud::start_trial(&tdb, &url, "کافه تست", "09121234567").await.unwrap();
    assert!(st.valid && st.mode == "online", "{}", st.message);
    assert!(st.user.starts_with("trial-"));
    assert!(cloud::start_trial(&tmp_db("trial2"), &url, "کافه تست", "09121234567").await.is_err(), "one trial per PC");
    // another PC, same phone: refused too
    let r = reqwest::Client::new()
        .post(format!("{url}/api/v1/auth/trial"))
        .json(&json!({ "machineId": "another-pc", "name": "کافه دو", "phone": "0912-123-4567" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403, "one trial per phone");
    assert_eq!(cloud::admin_call(&adm, "GET", "/trial", None).await.unwrap()["used"], 1);

    // ---------- offline half of the panel: registry + HS2 licence ----------
    let (code, cid) = admin::issue(&adm, 0, &machine, "کافه آفلاین", "سالانه", "2099-01-01").unwrap();
    let (_code2, cid2) = admin::issue(&adm, 0, &machine, "کافه آفلاین", "سالانه", "2099-06-01").unwrap();
    assert_eq!(cid, cid2, "same PC → same customer card");
    let c = admin::customer(&adm, cid).unwrap();
    assert_eq!((c.licenses, c.expires.as_str()), (2, "2099-06-01"));
    assert_eq!(admin::licences(&adm, cid, 10).unwrap().len(), 2);
    assert_eq!(admin::stats(&adm).unwrap().customers, 1);
    assert!(admin::customers_csv(&adm).unwrap().contains("کافه آفلاین"));

    let off = tmp_db("offline");
    let st = license::activate(&off, &code).unwrap();
    assert!(st.valid && st.mode == "offline", "{}", st.message);
    assert_eq!(st.customer, "کافه آفلاین");
    // an offline copy has no server quota
    cloud::consume(&off, "scan", 5).await.unwrap();
}

#[test]
fn admin_registry_rules() {
    let db = tmp_db("registry");
    assert!(admin::customer_save(&db, &admin::Customer { name: " ".into(), ..Default::default() }).is_err());
    assert!(admin::customer_save(&db, &admin::Customer { name: "x".into(), machine: "123".into(), ..Default::default() }).is_err());
    let soon = db.date_shift(&db.today(), 5);
    let id = admin::customer_save(&db, &admin::Customer { name: "الف".into(), expires: soon, ..Default::default() }).unwrap();
    admin::customer_save(&db, &admin::Customer { name: "ب".into(), expires: "2021-01-01".into(), ..Default::default() }).unwrap();
    admin::customer_save(&db, &admin::Customer { name: "ج".into(), status: "paused".into(), ..Default::default() }).unwrap();
    let names = |v: &str| admin::customers(&db, "", v).unwrap().into_iter().map(|c| c.name).collect::<Vec<_>>();
    assert_eq!(names("expiring"), ["الف"]);
    assert_eq!(names("expired"), ["ب"]);
    assert_eq!(names("paused"), ["ج"]);
    assert_eq!(admin::customers(&db, "الف", "all").unwrap().len(), 1);
    let s = admin::stats(&db).unwrap();
    assert_eq!((s.customers, s.expiring, s.expired), (3, 1, 1));
    admin::customer_delete(&db, id).unwrap();
    assert!(admin::customer(&db, id).is_err());
}
