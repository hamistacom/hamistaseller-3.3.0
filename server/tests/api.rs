//! Full-flow integration test: drives the real router in-process via `oneshot` (no TCP socket).
//! Same handlers, auth, DB and licence signing the live server uses.
use axum::body::Body;
use axum::extract::connect_info::MockConnectInfo;
use axum::http::{Request, StatusCode};
use axum::Router;
use hamista_cloud::app;
use hamista_cloud::state::{AppState, Config};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::path::PathBuf;
use tower::ServiceExt;

struct Api {
    app: Router,
    data_dir: PathBuf,
}

impl Api {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("hc-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::env::set_var("HAMISTA_ADMIN_USER", "admin");
        std::env::set_var("HAMISTA_ADMIN_PASS", "admin123");
        let cfg = Config { bind: "127.0.0.1:0".into(), data_dir: dir.clone(), token_ttl_hours: 72, agent_min_version: "1.0.0".into() };
        let state = AppState::new(cfg).unwrap();
        app::seed_admin(&state);
        let app = app::build(state).layer(MockConnectInfo(SocketAddr::from(([127, 0, 0, 1], 9999))));
        Api { app, data_dir: dir }
    }

    async fn req(&self, method: &str, path: &str, body: Option<Value>, bearer: Option<&str>, admin: Option<&str>) -> (StatusCode, Value) {
        let mut b = Request::builder().method(method).uri(path);
        if body.is_some() {
            b = b.header("content-type", "application/json");
        }
        if let Some(t) = bearer {
            b = b.header("authorization", format!("Bearer {t}"));
        }
        if let Some(a) = admin {
            b = b.header("x-admin-token", a);
        }
        let body = body.map(|v| Body::from(v.to_string())).unwrap_or(Body::empty());
        let resp = self.app.clone().oneshot(b.body(body).unwrap()).await.unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let val: Value = serde_json::from_slice(&bytes).unwrap_or(json!({ "_raw": String::from_utf8_lossy(&bytes) }));
        (status, val)
    }
    async fn post(&self, p: &str, b: Value, t: Option<&str>, a: Option<&str>) -> (StatusCode, Value) {
        self.req("POST", p, Some(b), t, a).await
    }
    async fn get(&self, p: &str, t: Option<&str>, a: Option<&str>) -> (StatusCode, Value) {
        self.req("GET", p, None, t, a).await
    }
    fn public_key(&self) -> String {
        std::fs::read_to_string(self.data_dir.join("signing-key.txt"))
            .unwrap()
            .lines()
            .find_map(|l| l.strip_prefix("public_key:").map(|s| s.trim().to_string()))
            .unwrap()
    }
}

#[tokio::test]
async fn full_flow() {
    let api = Api::new();

    // admin login
    let (s, r) = api.post("/api/v1/admin/login", json!({"username":"admin","password":"admin123"}), None, None).await;
    assert_eq!(s, 200, "{r}");
    let at = r["token"].as_str().unwrap().to_string();
    let (s, _) = api.post("/api/v1/admin/login", json!({"username":"admin","password":"bad"}), None, None).await;
    assert_eq!(s, 401);

    // plan with megabank + scan quota 5, 2 devices
    let (s, r) = api
        .post("/api/v1/admin/plans", json!({"name":"طلایی","maxDevices":2,"scanQuota":5,"sources":"maps,neshan","megabank":true}), None, Some(&at))
        .await;
    assert_eq!(s, 200, "{r}");
    let plan = r["id"].as_i64().unwrap();

    // create customer
    let (s, r) = api
        .post("/api/v1/admin/users", json!({"username":"cafe1","password":"secret1","customerName":"کافه لمیز","planId":plan,"expires":"2030-01-01"}), None, Some(&at))
        .await;
    assert_eq!(s, 200, "{r}");
    let uid = r["id"].as_i64().unwrap();
    let (s, _) = api.post("/api/v1/admin/users", json!({"username":"cafe1","password":"xxxxxx"}), None, Some(&at)).await;
    assert_eq!(s, 400, "duplicate username must be rejected");

    // client login
    let (s, r) = api
        .post("/api/v1/auth/login", json!({"username":"cafe1","password":"secret1","machineId":"PC-AAA","machineName":"laptop","appVersion":"1.0.0"}), None, None)
        .await;
    assert_eq!(s, 200, "{r}");
    let tok = r["token"].as_str().unwrap().to_string();
    let machine = r["machine"].as_str().unwrap().to_string();
    assert_eq!(r["publicKey"].as_str().unwrap(), api.public_key());
    assert_eq!(r["license"]["megabank"], json!(true));
    assert_eq!(r["license"]["scanLeft"], json!(5));

    // token verifies against the published public key, bound to this machine (what the desktop does offline)
    let claims = hamista_cloud::crypto::verify_token(&api.public_key(), &tok).unwrap();
    assert_eq!(claims.user, "cafe1");
    assert_eq!(claims.machine, machine);
    assert!(claims.exp - claims.iat <= 72 * 3600 + 5 && claims.exp > claims.iat);
    assert!(hamista_cloud::crypto::verify_token(&"0".repeat(64), &tok).is_err(), "wrong key must fail");

    let (s, _) = api.post("/api/v1/auth/login", json!({"username":"cafe1","password":"no","machineId":"PC-AAA"}), None, None).await;
    assert_eq!(s, 401);

    // device cap = 2: 2nd ok, 3rd blocked
    let (s, _) = api.post("/api/v1/auth/login", json!({"username":"cafe1","password":"secret1","machineId":"PC-BBB"}), None, None).await;
    assert_eq!(s, 200);
    let (s, r) = api.post("/api/v1/auth/login", json!({"username":"cafe1","password":"secret1","machineId":"PC-CCC"}), None, None).await;
    assert_eq!(s, 403, "device cap: {r}");

    // heartbeat refreshes the token (iat has 1-second resolution)
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    let (s, r) = api.post("/api/v1/license/heartbeat", json!({"appVersion":"1.0.1"}), Some(&tok), None).await;
    assert_eq!(s, 200);
    assert!(r["token"].as_str().unwrap() != tok);

    // quota: 5 then 429
    let mut last = json!(null);
    for _ in 0..5 {
        let (s, r) = api.post("/api/v1/quota/consume", json!({"kind":"scan","n":1}), Some(&tok), None).await;
        assert_eq!(s, 200);
        last = r;
    }
    assert_eq!(last["scanLeft"], json!(0));
    let (s, _) = api.post("/api/v1/quota/consume", json!({"kind":"scan","n":1}), Some(&tok), None).await;
    assert_eq!(s, 429, "quota must be enforced server-side");

    // concurrency: fire 20 consume requests at once on a fresh user with scanQuota=5 -> exactly 5 succeed
    {
        let (_s, r) = api.post("/api/v1/admin/users", json!({"username":"race1","password":"secret1","planId":plan,"expires":"2030-01-01"}), None, Some(&at)).await;
        assert_eq!(_s, 200, "{r}");
        let (_s, r) = api.post("/api/v1/auth/login", json!({"username":"race1","password":"secret1","machineId":"R1"}), None, None).await;
        let rtok = r["token"].as_str().unwrap().to_string();
        let mut set = tokio::task::JoinSet::new();
        for _ in 0..20 {
            let api2 = api.app.clone();
            let t = rtok.clone();
            set.spawn(async move {
                let req = Request::builder().method("POST").uri("/api/v1/quota/consume").header("content-type","application/json")
                    .header("authorization", format!("Bearer {t}")).body(Body::from(json!({"kind":"scan","n":1}).to_string())).unwrap();
                api2.oneshot(req).await.unwrap().status()
            });
        }
        let mut okc = 0;
        while let Some(res) = set.join_next().await { if res.unwrap() == 200 { okc += 1; } }
        assert_eq!(okc, 5, "atomic quota: exactly the cap must succeed under concurrency, got {okc}");
    }

    // mega-bank ingest (public business data) + merge
    let places = json!({"places":[
        {"placeKey":"0x1:0xA","name":"کافه لمیز","mobiles":["09120000001"],"city":"تهران","province":"تهران","category":"کافه","source":"maps","website":"https://lamiz.ir"},
        {"placeKey":"0x2:0xB","name":"رستوران شب","mobiles":["09120000002"],"city":"اصفهان","province":"اصفهان","category":"رستوران","source":"maps"}]});
    let (s, r) = api.post("/api/v1/megabank/ingest", places, Some(&tok), None).await;
    assert_eq!(s, 200);
    assert_eq!(r["added"], json!(2));
    let (_s, r) = api.post("/api/v1/megabank/ingest", json!({"places":[{"placeKey":"0x1:0xA","mobiles":["09120000009"]}]}), Some(&tok), None).await;
    assert_eq!((r["added"].as_i64(), r["merged"].as_i64()), (Some(0), Some(1)));

    // client search (plan allows) — union kept both numbers for the merged row
    let (s, r) = api.post("/api/v1/megabank/search", json!({"province":"تهران"}), Some(&tok), None).await;
    assert_eq!(s, 200);
    assert_eq!(r["total"], json!(1));
    assert!(r["rows"][0]["mobiles"].as_str().unwrap().contains("09120000009"));

    // admin view: all + facets + csv export
    let (_s, r) = api.post("/api/v1/admin/megabank/search", json!({}), None, Some(&at)).await;
    assert_eq!(r["total"], json!(2));
    let (_s, r) = api.post("/api/v1/admin/megabank/facets", json!({}), None, Some(&at)).await;
    let provinces: std::collections::HashMap<String, i64> = r["provinces"].as_array().unwrap().iter().map(|p| (p[0].as_str().unwrap().to_string(), p[1].as_i64().unwrap())).collect();
    assert_eq!(provinces.get("تهران"), Some(&1));
    let (s, r) = api.req("POST", "/api/v1/admin/megabank/export", Some(json!({})), None, Some(&at)).await;
    assert_eq!(s, 200);
    assert!(r["_raw"].as_str().unwrap().contains("کافه لمیز"));

    // suspend -> client blocked; reactivate
    let (s, _) = api.req("PUT", &format!("/api/v1/admin/users/{uid}"), Some(json!({"status":"suspended"})), None, Some(&at)).await;
    assert_eq!(s, 200);
    let (s, _) = api.post("/api/v1/license/heartbeat", json!({}), Some(&tok), None).await;
    assert_eq!(s, 403, "suspended account must be refused");
    api.req("PUT", &format!("/api/v1/admin/users/{uid}"), Some(json!({"status":"active"})), None, Some(&at)).await;

    // revoke device -> that device blocked
    let (_s, r) = api.get(&format!("/api/v1/admin/users/{uid}/devices"), None, Some(&at)).await;
    let dev_aaa = r["items"].as_array().unwrap().iter().find(|d| d["name"] == json!("laptop")).unwrap()["id"].as_i64().unwrap();
    api.post(&format!("/api/v1/admin/devices/{dev_aaa}/revoke"), json!({}), None, Some(&at)).await;
    let (s, _) = api.post("/api/v1/license/heartbeat", json!({}), Some(&tok), None).await;
    assert_eq!(s, 403, "revoked device must be refused at next heartbeat");

    // notifications
    api.post("/api/v1/admin/notifications", json!({"title":"خوش آمدید","audience":"all"}), None, Some(&at)).await;
    // use the 2nd device's token (PC-BBB still valid) to read notifications
    let (_s, r) = api.post("/api/v1/auth/login", json!({"username":"cafe1","password":"secret1","machineId":"PC-BBB"}), None, None).await;
    let tok2 = r["token"].as_str().unwrap().to_string();
    let (s, r) = api.get("/api/v1/notifications", Some(&tok2), None).await;
    assert_eq!(s, 200);
    assert_eq!(r["items"].as_array().unwrap().len(), 1);
    assert_eq!(r["items"][0]["read"], json!(false));
    let nid = r["items"][0]["id"].as_i64().unwrap();
    api.post("/api/v1/notifications/read", json!({"id":nid}), Some(&tok2), None).await;
    let (_s, r) = api.get("/api/v1/notifications", Some(&tok2), None).await;
    assert_eq!(r["items"][0]["read"], json!(true));

    // agent version + scenarios (public) with version bump
    api.req("PUT", "/api/v1/admin/version", Some(json!({"version":"1.2.0","notes":"نو"})), None, Some(&at)).await;
    let (_s, r) = api.get("/api/v1/agent/version", None, None).await;
    assert_eq!(r["version"], json!("1.2.0"));
    api.req("PUT", "/api/v1/admin/scenarios/maps", Some(json!({"body":{"steps":["open","read"]}})), None, Some(&at)).await;
    let (_s, r) = api.get("/api/v1/scenarios/maps", None, None).await;
    assert_eq!(r["version"], json!(1));
    assert_eq!(r["body"]["steps"], json!(["open", "read"]));
    let (_s, r) = api.req("PUT", "/api/v1/admin/scenarios/maps", Some(json!({"body":{"steps":["x"]}})), None, Some(&at)).await;
    assert_eq!(r["version"], json!(2), "editing a scenario bumps its version");

    // overview + no-auth guard
    let (_s, r) = api.get("/api/v1/admin/overview", None, Some(&at)).await;
    assert_eq!(r["users"], json!(2)); // cafe1 + race1
    assert_eq!(r["megabank"], json!(2));
    let (s, _) = api.post("/api/v1/megabank/search", json!({}), None, None).await;
    assert_eq!(s, 401, "no token must be rejected");

    let _ = std::fs::remove_dir_all(&api.data_dir);
}
