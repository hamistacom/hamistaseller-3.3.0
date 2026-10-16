//! Official sending channel, driven from inside the app (no browser): any HTTP SMS panel
//! (described as data; presets live in the UI). Messengers (WhatsApp, Telegram, Eitaa, Bale) go
//! through the chat assistant instead — the user's own logged-in account, one chat at a time.
use crate::commands::account_secret;
use crate::db::{Db, Res};
use crate::logic::{accepted, intl_mobile, pct_encode, render};
use serde::Deserialize;
use serde_json::Value;

/// A failed send. `recipient` = the problem is this one recipient (bad number, no Bale account),
/// so it must not count toward the "5 failures in a row → stop" rule.
pub struct SendErr {
    pub msg: String,
    pub recipient: bool,
    pub delivery: &'static str, // stored on the contact row: failed | invalid | no_account
}

impl SendErr {
    fn account(msg: impl Into<String>) -> Self {
        SendErr {
            msg: msg.into(),
            recipient: false,
            delivery: "failed",
        }
    }
    fn recipient(msg: impl Into<String>, delivery: &'static str) -> Self {
        SendErr {
            msg: msg.into(),
            recipient: true,
            delivery,
        }
    }
}

type Sent = Result<String, SendErr>; // Ok(provider message id, may be empty)

fn snippet(s: &str) -> String {
    s.chars().take(200).collect()
}

// ---------------- SMS panels ----------------

/// Any HTTP SMS gateway, described as data.
/// Placeholders: {apikey} {username} {password} {from} {to} (09…) {to98} (989…) {text}.
/// In JSON bodies a value written as "{#from}" is sent as a number.
#[derive(Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct SmsAccount {
    pub method: String, // GET | POST
    pub url: String,
    pub body_type: String, // json | form | none
    pub body: Value,
    pub headers: Vec<(String, String)>,
    pub api_key: String,
    pub username: String,
    pub password: String,
    pub sender: String,
    pub ok_path: String,        // "" = any 2xx; "." = whole body; "a.b.0" = JSON path
    pub ok_values: Vec<String>, // exact matches, or ">N" for a number greater than N
}

fn fill(tpl: &str, vars: &[(&str, &str)], encode: bool) -> String {
    let enc: Vec<(&str, String)> = vars.iter().map(|(k, v)| (*k, if encode { pct_encode(v) } else { v.to_string() })).collect();
    render(tpl, &enc.iter().map(|(k, v)| (*k, v.as_str())).collect::<Vec<_>>())
}

fn fill_json(v: &Value, vars: &[(&str, &str)]) -> Value {
    match v {
        Value::String(s) if s.starts_with("{#") && s.ends_with('}') => {
            let raw = fill(&format!("{{{}", &s[2..]), vars, false);
            raw.parse::<i64>().map(Value::from).unwrap_or(Value::String(raw))
        }
        Value::String(s) => Value::String(fill(s, vars, false)),
        Value::Array(a) => Value::Array(a.iter().map(|x| fill_json(x, vars)).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), fill_json(x, vars))).collect()),
        other => other.clone(),
    }
}

/// Dot path with array indexes; keys match case-insensitively (`IsSuccess` == `isSuccess`).
fn json_at<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(v, |v, k| match (v, k.parse::<usize>()) {
        (Value::Array(a), Ok(i)) => a.get(i),
        (Value::Object(o), _) => o.get(k).or_else(|| o.iter().find(|(kk, _)| kk.eq_ignore_ascii_case(k)).map(|(_, x)| x)),
        _ => None,
    })
}

async fn send_sms(http: &reqwest::Client, a: &SmsAccount, to: &str, text: &str) -> Sent {
    let to98 = intl_mobile(to);
    if to98.is_empty() {
        return Err(SendErr::recipient("شماره موبایل نامعتبر است.", "invalid"));
    }
    let vars = [
        ("apikey", a.api_key.as_str()),
        ("username", &a.username),
        ("password", &a.password),
        ("from", &a.sender),
        ("to", to),
        ("to98", &to98),
        ("text", text),
    ];
    let method = if a.method.eq_ignore_ascii_case("GET") {
        reqwest::Method::GET
    } else {
        reqwest::Method::POST
    };
    let mut rb = http.request(method, fill(&a.url, &vars, true));
    for (k, v) in &a.headers {
        rb = rb.header(k.as_str(), fill(v, &vars, false));
    }
    rb = match a.body_type.as_str() {
        "json" => rb.json(&fill_json(&a.body, &vars)),
        "form" => {
            let form: Vec<(String, String)> = a
                .body
                .as_object()
                .map(|o| {
                    o.iter()
                        .map(|(k, v)| (k.clone(), fill(v.as_str().unwrap_or(&v.to_string()), &vars, false)))
                        .collect()
                })
                .unwrap_or_default();
            rb.form(&form)
        }
        _ => rb,
    };
    let resp = rb.send().await.map_err(|e| SendErr::account(format!("اتصال به پنل پیامک ناموفق بود: {e}")))?;
    let code = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !code.is_success() {
        return Err(SendErr::account(format!("HTTP {}: {}", code.as_u16(), snippet(&body))));
    }
    if a.ok_path.trim().is_empty() {
        return Ok(String::new());
    }
    let got = if a.ok_path == "." {
        body.clone()
    } else {
        let v: Value = serde_json::from_str(&body).map_err(|_| SendErr::account(format!("پاسخ نامعتبر: {}", snippet(&body))))?;
        json_at(&v, &a.ok_path)
            .map(|x| x.as_str().map(String::from).unwrap_or_else(|| x.to_string()))
            .unwrap_or_default()
    };
    if accepted(&got, &a.ok_values) {
        Ok(String::new())
    } else {
        Err(SendErr::account(format!("پنل پیامک خطا داد: {}", snippet(&body))))
    }
}

// ---------------- email ----------------

// ---------------- loading an account ----------------

pub enum Sender {
    Sms(SmsAccount, reqwest::Client),
}

pub struct Loaded {
    pub sender: Sender,
    pub id: String,
    pub kind: String, // sms — also the `messages.channel` value
    pub name: String,
    pub per_minute: i64,
}

/// Account config (public) + decrypted secret, merged into one typed account.
fn typed<T: serde::de::DeserializeOwned>(db: &Db, a: &crate::db::Account) -> Res<T> {
    let mut obj = a.config.as_object().cloned().unwrap_or_default();
    obj.extend(account_secret(db, &a.id)?);
    serde_json::from_value(Value::Object(obj)).map_err(|e| format!("تنظیمات حساب «{}» ناقص است: {e}", a.name))
}

pub fn load(db: &Db, id: &str) -> Res<Loaded> {
    let a = db
        .accounts()?
        .into_iter()
        .find(|a| a.id == id)
        .ok_or("حساب ارسال انتخاب نشده یا حذف شده است.")?;
    let sender = match a.kind.as_str() {
        "sms" => {
            let s: SmsAccount = typed(db, &a)?;
            if s.url.trim().is_empty() {
                return Err("آدرس API پنل پیامک خالی است.".into());
            }
            Sender::Sms(s, crate::enrich::client())
        }
        _ => return Err("این نوع حساب در این نسخه پشتیبانی نمی‌شود؛ فقط پنل پیامک. برای پیام‌رسان‌ها از «دستیار گفتگو» استفاده کنید.".into()),
    };
    Ok(Loaded {
        sender,
        id: a.id,
        kind: a.kind,
        name: a.name,
        per_minute: a.per_minute,
    })
}

impl Sender {
    /// Which contact kind this sender delivers to.
    pub fn target(&self) -> &'static str {
        "mobile"
    }

    pub async fn send(&self, to: &str, _subject: &str, body: &str, _request_id: &str) -> Sent {
        match self {
            Sender::Sms(a, http) => send_sms(http, a, to, body).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn templates_and_paths() {
        let vars = [("from", "3000"), ("to", "0912"), ("text", "سلام")];
        assert_eq!(fill("u?to={to}&m={text}", &vars, true), "u?to=0912&m=%D8%B3%D9%84%D8%A7%D9%85");
        let body = fill_json(&json!({"sender": "{#from}", "msg": "{text}", "list": ["{to}"]}), &vars);
        assert_eq!(body, json!({"sender": 3000, "msg": "سلام", "list": ["0912"]}));
        let v = json!({"Return": {"Status": 200}, "entries": [{"id": 5}]});
        assert_eq!(json_at(&v, "return.status"), Some(&json!(200)));
        assert_eq!(json_at(&v, "entries.0.id"), Some(&json!(5)));
    }
}
