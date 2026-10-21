//! Outside alerts for the price watch: a Telegram bot message or a text message through the configured
//! SMS panel. The bot token is sealed like every other secret; the SMS panel is a «حساب ارسال» of the app.
use crate::buybox::{BuyBox, LOST, WON};
use crate::db::{Db, Res};
use serde::{Deserialize, Serialize};

const TG: &str = "https://api.telegram.org";

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub telegram_chat: String,
    /// View only: a bot token is stored (the token itself never leaves the backend).
    pub has_token: bool,
    pub sms_account: String,
    pub sms_to: String,
}

pub fn config(db: &Db) -> Config {
    let g = |k: &str| db.kv_get(k).unwrap_or_default();
    Config {
        telegram_chat: g("alert_tg_chat"),
        has_token: !g("alert_tg_token").is_empty(),
        sms_account: g("alert_sms_account"),
        sms_to: g("alert_sms_to"),
    }
}

/// `token`: None keeps the stored one, Some("") removes it.
pub fn save(db: &Db, c: &Config, token: Option<&str>) -> Res<()> {
    let chat = c.telegram_chat.trim();
    if chat.chars().count() > 64 || !chat.chars().all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '@')) {
        return Err("شناسه چت تلگرام نامعتبر است (عدد یا @نام‌کانال).".into());
    }
    let to = crate::logic::ascii_digits(c.sms_to.trim());
    if !to.is_empty() && !(to.len() == 11 && to.starts_with("09") && to.bytes().all(|b| b.is_ascii_digit())) {
        return Err("شماره موبایل گیرنده را به شکل 09123456789 بنویسید.".into());
    }
    match token {
        Some("") => db.c().execute("DELETE FROM kv WHERE k='alert_tg_token'", []).map(|_| ()).map_err(crate::db::err)?,
        Some(t) => {
            let t = t.trim();
            if t.len() > 120 || !t.contains(':') || t.chars().any(|ch| ch.is_whitespace()) {
                return Err("توکن ربات تلگرام معتبر نیست (شکل 123456:ABC…).".into());
            }
            db.kv_set("alert_tg_token", &crate::logic::b64_encode(&crate::secrets::seal_str(t)?))?;
        }
        None => {}
    }
    db.kv_set("alert_tg_chat", chat)?;
    db.kv_set("alert_sms_account", c.sms_account.trim())?;
    db.kv_set("alert_sms_to", &to)
}

/// Worth telling the seller: the buy box was lost, or won back.
pub fn should_notify(prev: &str, now: &str) -> bool {
    prev != now && (now == LOST || (now == WON && prev == LOST))
}

pub fn message(name: &str, b: &BuyBox) -> String {
    let fa = crate::logic::fa_num;
    let head = if b.status == WON { "✅ بای‌باکس را پس گرفتید" } else { "⚠️ بای‌باکس را از دست دادید" };
    let mut t = format!("{head}\n«{name}»");
    if b.status == LOST && b.winner_price > 0 {
        t.push_str(&format!("\nبرنده: {} با {} تومان (قیمت شما {})", if b.winner_seller.is_empty() { "—" } else { &b.winner_seller }, fa(b.winner_price as f64), fa(b.my_price as f64)));
    }
    if !b.advice.is_empty() {
        t.push_str(&format!("\n{}", b.advice));
    }
    t
}

async fn telegram(db: &Db, text: &str) -> Res<()> {
    let token = db
        .kv_get("alert_tg_token")
        .and_then(|v| crate::logic::b64_decode(&v))
        .and_then(|v| crate::secrets::open_str(&v).ok())
        .ok_or("توکن ربات تلگرام ثبت نشده است.")?;
    let chat = db.kv_get("alert_tg_chat").filter(|c| !c.is_empty()).ok_or("شناسه چت تلگرام ثبت نشده است.")?;
    let r = crate::enrich::client()
        .post(format!("{TG}/bot{token}/sendMessage"))
        .json(&serde_json::json!({"chat_id": chat, "text": text, "disable_web_page_preview": true}))
        .send()
        .await
        // the error text carries the URL, which holds the token
        .map_err(|e| format!("اتصال به تلگرام برقرار نشد: {}", e.without_url()))?;
    if r.status().is_success() {
        Ok(())
    } else {
        Err(format!("تلگرام پیام را نپذیرفت (کد {}). توکن و شناسه چت را بررسی کنید.", r.status().as_u16()))
    }
}

async fn sms(db: &Db, text: &str) -> Res<()> {
    let id = db.kv_get("alert_sms_account").filter(|c| !c.is_empty()).ok_or("پنل پیامک برای هشدار انتخاب نشده است.")?;
    let to = db.kv_get("alert_sms_to").filter(|c| !c.is_empty()).ok_or("شماره گیرنده هشدار ثبت نشده است.")?;
    let acc = crate::channels::load(db, &id)?;
    acc.sender.send(&to, "", text, "alert").await.map(|_| ()).map_err(|e| e.msg)
}

/// Sends `text` on `channel` (telegram | sms | both). Ok = every wanted channel delivered; Err names what failed.
pub async fn send(db: &Db, channel: &str, text: &str) -> Res<()> {
    let mut errs = vec![];
    if matches!(channel, "telegram" | "both") {
        if let Err(e) = telegram(db, text).await {
            errs.push(e);
        }
    }
    if matches!(channel, "sms" | "both") {
        if let Err(e) = sms(db, text).await {
            errs.push(e);
        }
    }
    if errs.is_empty() {
        Ok(())
    } else {
        Err(errs.join(" · "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_losses_and_comebacks_are_sent() {
        assert!(should_notify("", LOST));
        assert!(should_notify(WON, LOST));
        assert!(should_notify(LOST, WON));
        assert!(!should_notify(LOST, LOST), "no repeat every day");
        assert!(!should_notify("", WON));
        assert!(!should_notify(WON, "not_listed"));
    }

    #[test]
    fn message_names_the_winner() {
        let b = BuyBox {
            status: LOST.into(),
            my_price: 420_000,
            winner_seller: "رقیب".into(),
            winner_price: 400_000,
            advice: "قیمت را روی ۳۹۸٬۰۰۰ بگذارید".into(),
            ..Default::default()
        };
        let m = message("عسل ۵۰۰ گرمی", &b);
        assert!(m.contains("از دست دادید") && m.contains("رقیب") && m.contains("عسل ۵۰۰ گرمی") && m.contains("بگذارید"), "{m}");
        assert!(message("x", &BuyBox { status: WON.into(), ..Default::default() }).contains("پس گرفتید"));
    }
}
