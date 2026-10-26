use serde::Serialize;
use serde_json::Value;
use std::{sync::{atomic::{AtomicBool, Ordering}, OnceLock}, time::Duration};
use tauri::{AppHandle, Manager, State};

const MAX_RESPONSE: usize = 256 * 1024;

#[derive(Default)]
pub struct LookupState {
    client: OnceLock<reqwest::Client>,
    busy: AtomicBool,
}

struct Flight<'a>(&'a AtomicBool);

impl<'a> Flight<'a> {
    fn acquire(flag: &'a AtomicBool) -> Result<Self, String> {
        flag.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| Self(flag)).map_err(|_| "Translation busy".into())
    }
}

impl Drop for Flight<'_> {
    fn drop(&mut self) { self.0.store(false, Ordering::Release); }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Meaning {
    part_of_speech: String,
    definition: String,
    example: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupResult {
    text: String,
    source: String,
    phonetic: String,
    meanings: Vec<Meaning>,
}

fn validate(enabled: bool, mode: &str, text: &str, direction: &str) -> Result<(), String> {
    if !enabled { return Err("Translation disabled".into()); }
    if text.trim().is_empty() { return Err("Translation empty".into()); }
    if !matches!(mode, "translate" | "dictionary") || !matches!(direction, "en-zh" | "zh-en")
        || text.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')) {
        return Err("Translation invalid request".into());
    }
    if mode == "translate" && text.len() > 500 { return Err("Translation too long".into()); }
    if mode == "dictionary" && (text.len() > 80 || !text.bytes().any(|c| c.is_ascii_alphabetic())
        || !text.bytes().all(|c| c.is_ascii_alphabetic() || matches!(c, b'-' | b'\''))) {
        return Err("Translation invalid word".into());
    }
    Ok(())
}

fn bounded_text(value: &Value, limit: usize) -> String {
    value.as_str().unwrap_or_default().chars().take(limit).collect::<String>().trim().to_string()
}

fn parse_translation(value: &Value) -> Result<LookupResult, String> {
    if value["quotaFinished"].as_bool() == Some(true) { return Err("Translation quota".into()); }
    if value["responseStatus"].as_u64() != Some(200) && value["responseStatus"].as_str() != Some("200") {
        return Err("Translation unavailable".into());
    }
    let text = bounded_text(&value["responseData"]["translatedText"], 8192);
    if text.is_empty() { return Err("Translation invalid response".into()); }
    Ok(LookupResult { text, source: "MyMemory".into(), phonetic: String::new(), meanings: vec![] })
}

fn parse_dictionary(value: &Value) -> Result<LookupResult, String> {
    let entries = value.as_array().ok_or("Translation invalid response")?;
    let entry = entries.first().ok_or("Translation word not found")?;
    let text = bounded_text(&entry["word"], 128);
    let mut phonetic = bounded_text(&entry["phonetic"], 128);
    if phonetic.is_empty() {
        if let Some(items) = entry["phonetics"].as_array() {
            phonetic = items.iter().map(|item| bounded_text(&item["text"], 128)).find(|s| !s.is_empty()).unwrap_or_default();
        }
    }
    let mut meanings = Vec::new();
    if let Some(items) = entry["meanings"].as_array() {
        for item in items {
            if let Some(definitions) = item["definitions"].as_array() {
                for definition in definitions {
                    let text = bounded_text(&definition["definition"], 2048);
                    if !text.is_empty() {
                        meanings.push(Meaning { part_of_speech: bounded_text(&item["partOfSpeech"], 80), definition: text, example: bounded_text(&definition["example"], 2048) });
                    }
                    if meanings.len() == 6 { break; }
                }
            }
            if meanings.len() == 6 { break; }
        }
    }
    if text.is_empty() || meanings.is_empty() { return Err("Translation invalid response".into()); }
    let license = bounded_text(&entry["license"]["name"], 100);
    let source_url = entry["sourceUrls"].as_array().and_then(|urls| urls.first())
        .map(|url| bounded_text(url, 512)).unwrap_or_default();
    let mut source = "Free Dictionary API".to_string();
    if !license.is_empty() { source.push_str(&format!(" · {license}")); }
    if source_url.starts_with("https://en.wiktionary.org/wiki/") { source.push_str(&format!(" · {source_url}")); }
    Ok(LookupResult { text, source, phonetic, meanings })
}

fn enabled(app: &AppHandle) -> Result<bool, String> {
    let state = app.state::<crate::service::AppState>();
    let store = state.store.lock().map_err(|_| "Storage is unavailable")?;
    Ok(store.settings().translation_enabled)
}

fn network_error(error: reqwest::Error) -> String {
    // 错误中可能带有完整查询 URL；只返回固定标识，不泄露或记录原始文本。
    if error.is_timeout() { "Translation timeout" } else { "Translation unavailable" }.into()
}

async fn read_json(mut response: reqwest::Response, dictionary: bool) -> Result<Value, String> {
    let status = response.status();
    if status.as_u16() == 429 { return Err("Translation quota".into()); }
    if dictionary && status.as_u16() == 404 { return Err("Translation word not found".into()); }
    if !status.is_success() { return Err("Translation unavailable".into()); }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(network_error)? {
        if body.len().saturating_add(chunk.len()) > MAX_RESPONSE { return Err("Translation invalid response".into()); }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| "Translation invalid response".into())
}

#[tauri::command]
pub async fn lookup_translation(app: AppHandle, state: State<'_, LookupState>, mode: String, text: String, direction: String) -> Result<LookupResult, String> {
    let text = text.trim();
    validate(enabled(&app)?, &mode, text, &direction)?;
    let _flight = Flight::acquire(&state.busy)?;
    if state.client.get().is_none() {
        let client = reqwest::Client::builder().https_only(true)
            .redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5)).user_agent(concat!("SuperSearch/", env!("CARGO_PKG_VERSION")))
            .build().map_err(network_error)?;
        let _ = state.client.set(client);
    }
    let client = state.client.get().ok_or("Translation unavailable")?;
    // 只允许固定公开服务；等待网络时不持有本地搜索缓存或数据库锁。
    let dictionary = mode == "dictionary";
    let request = if dictionary {
        client.get(format!("https://api.dictionaryapi.dev/api/v2/entries/en/{}", text.to_ascii_lowercase()))
    } else {
        let pair = if direction == "zh-en" { "zh-CN|en" } else { "en|zh-CN" };
        client.get("https://api.mymemory.translated.net/get").query(&[("q", text), ("langpair", pair)])
    };
    let data = read_json(request.send().await.map_err(network_error)?, dictionary).await?;
    if !enabled(&app)? { return Err("Translation disabled".into()); }
    if dictionary { parse_dictionary(&data) } else { parse_translation(&data) }
}

#[tauri::command]
pub fn copy_translation(app: AppHandle, text: String) -> Result<(), String> {
    if !enabled(&app)? { return Err("Translation disabled".into()); }
    if text.is_empty() || text.len() > 64 * 1024 || text.contains('\0') { return Err("Translation invalid request".into()); }
    crate::platform::copy_text(&text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn validates_opt_in_and_utf8_byte_limits() {
        assert_eq!(validate(false, "translate", "hello", "en-zh").unwrap_err(), "Translation disabled");
        assert!(validate(true, "translate", &"a".repeat(500), "en-zh").is_ok());
        assert!(validate(true, "translate", &"中".repeat(166), "zh-en").is_ok());
        assert!(validate(true, "translate", &"中".repeat(167), "zh-en").is_err());
        assert!(validate(true, "translate", "  ", "en-zh").is_err());
        assert!(validate(true, "translate", "hello", "auto").is_err());
        assert!(validate(true, "other", "hello", "en-zh").is_err());
        assert!(validate(true, "translate", "a\0b", "en-zh").is_err());
    }

    #[test]
    fn dictionary_accepts_only_a_bounded_english_word() {
        for word in ["hello", "mother-in-law", "don't"] {
            assert!(validate(true, "dictionary", word, "en-zh").is_ok());
        }
        for word in ["hello world", "你好", "../hello", "hello?x=1", "-", "''"] {
            assert!(validate(true, "dictionary", word, "en-zh").is_err());
        }
        assert!(validate(true, "dictionary", &"a".repeat(81), "en-zh").is_err());
    }

    #[test]
    fn translation_parser_rejects_quota_and_malformed_results() {
        let parsed = parse_translation(&json!({"responseStatus":200,"responseData":{"translatedText":"你好"}})).unwrap();
        assert_eq!(parsed.text, "你好");
        assert_eq!(parsed.source, "MyMemory");
        assert!(parse_translation(&json!({"responseStatus":"200","responseData":{"translatedText":"hello"}})).is_ok());
        assert_eq!(parse_translation(&json!({"quotaFinished":true,"responseStatus":403})).unwrap_err(), "Translation quota");
        for bad in [json!({}), json!({"responseStatus":500}), json!({"responseStatus":200,"responseData":{"translatedText":""}})] {
            assert!(parse_translation(&bad).is_err());
        }
    }

    #[test]
    fn dictionary_parser_bounds_results_and_ignores_remote_markup_fields() {
        let meaning = json!({"partOfSpeech":"noun","definitions":[{"definition":"a greeting","example":"hello there"}]});
        let data = json!([{"word":"hello","phonetics":[{"text":"həˈləʊ","audio":"https://untrusted.invalid/track"}],"meanings":vec![meaning;20]}]);
        let result = parse_dictionary(&data).unwrap();
        assert_eq!(result.text, "hello");
        assert_eq!(result.phonetic, "həˈləʊ");
        assert_eq!(result.meanings.len(), 6);
        assert_eq!(result.meanings[0].example, "hello there");
        assert!(!serde_json::to_string(&result).unwrap().contains("untrusted"));
        assert!(parse_dictionary(&json!([])).is_err());
        assert!(parse_dictionary(&json!([{"word":"hello"}])).is_err());
    }

    #[test]
    fn single_flight_guard_releases_on_drop() {
        let flag = std::sync::atomic::AtomicBool::new(false);
        let guard = Flight::acquire(&flag).unwrap();
        assert!(Flight::acquire(&flag).is_err());
        drop(guard);
        assert!(Flight::acquire(&flag).is_ok());
    }

    #[test]
    fn reads_bounded_http_responses_and_maps_provider_status() {
        use std::{io::{Read, Write}, net::TcpListener};
        fn reply(status: u16, body: &str, dictionary: bool) -> Result<Value, String> {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let wire = format!("HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
                let _ = stream.read(&mut [0u8; 4096]);
                // 超限时客户端会提前断开，服务端写入失败不改变断言。
                let _ = stream.write_all(wire.as_bytes());
            });
            let result = tauri::async_runtime::block_on(async {
                let client = reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(3)).build().unwrap();
                read_json(client.get(format!("http://{address}/")).send().await.unwrap(), dictionary).await
            });
            server.join().unwrap();
            result
        }
        assert_eq!(reply(200, r#"{"responseStatus":200}"#, false).unwrap()["responseStatus"], 200);
        assert_eq!(reply(429, "private server detail", false).unwrap_err(), "Translation quota");
        assert_eq!(reply(404, "missing", true).unwrap_err(), "Translation word not found");
        assert_eq!(reply(503, "private server detail", false).unwrap_err(), "Translation unavailable");
        assert!(reply(200, "not json", false).is_err());
        assert!(reply(200, &"x".repeat(MAX_RESPONSE + 1), false).is_err());
    }
}
