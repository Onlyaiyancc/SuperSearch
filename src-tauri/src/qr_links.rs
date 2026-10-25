use std::collections::HashSet;
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WebLink { pub id: String, pub url: String, pub domain: String }

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractedLinks { pub items: Vec<WebLink>, pub limited: bool }

fn text_delimiter(c: char) -> bool {
    c.is_whitespace() || matches!(c, '<' | '>' | '"' | '`' | '{' | '}' | '，' | '。' | '；' | '：' | '！' | '？' | '、' | '“' | '”' | '‘' | '’' | '（' | '）' | '【' | '】' | '《' | '》' | '「' | '」' | '『' | '』')
}

pub fn web_url(text: &str) -> Option<tauri::Url> {
    if text.len() > 16 * 1024 || text.trim() != text || text.chars().any(|c| c.is_control() || c.is_whitespace()) { return None; }
    let url = tauri::Url::parse(text).ok()?;
    (matches!(url.scheme(), "http" | "https") && url.host_str().is_some() && url.username().is_empty() && url.password().is_none()).then_some(url)
}

// 仅修复 OCR 在协议符号间插入的空格，不猜测域名里的相似字符。
fn screen_protocols(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut output = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let mut matched = None;
        for scheme in ["https", "http"] {
            let letters: Vec<char> = scheme.chars().collect();
            if i + letters.len() > chars.len() || !chars[i..i+letters.len()].iter().zip(&letters).all(|(a,b)| a.eq_ignore_ascii_case(b)) { continue; }
            let mut end = i + letters.len();
            let mut valid = true;
            for symbol in [':', '/', '/'] {
                while end < chars.len() && matches!(chars[end], ' ' | '\t') { end += 1; }
                if chars.get(end) != Some(&symbol) { valid = false; break; }
                end += 1;
            }
            if valid {
                while end < chars.len() && matches!(chars[end], ' ' | '\t') { end += 1; }
                matched = Some((scheme, end)); break;
            }
        }
        if let Some((scheme, end)) = matched { output.push_str(scheme); output.push_str("://"); i = end; }
        else { output.push(chars[i]); i += 1; }
    }
    output
}

pub fn extract_screen_text(text: &str) -> Result<ExtractedLinks, String> {
    if text.len() > 64*1024 || text.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')) {
        return Err("Conversion screen OCR text limit".into());
    }
    let mut items = Vec::new();
    let mut seen = HashSet::new();
    let mut limited = false;
    for line in text.lines() {
        let line = screen_protocols(line);
        // 按完整词边界处理长 OCR 行，绝不截断网址后生成另一个目标。
        for token in line.split(text_delimiter).filter(|token| !token.is_empty()) {
            if token.len() > 16*1024 { limited = true; continue; }
            let extracted = extract(token)?;
            limited |= extracted.limited;
            let mut links = extracted.items;
            if links.is_empty() && !token.contains('@') && !token.contains("://") {
                let bare = token.trim_matches(['(', ')', '[', ']', '\'', '.', ',', ';', '!', '?', ':']);
                let host = bare.split(['/', '?', '#', ':']).next().unwrap_or("");
                let suffix = host.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
                let supported = host.to_ascii_lowercase().starts_with("www.") || ["com", "org", "net", "edu", "gov", "io", "ai", "cn", "uk", "de", "jp", "co", "dev", "app", "info", "xyz", "me", "tv", "us", "fr", "au", "ca"].contains(&suffix.as_str());
                let valid_host = host.contains('.') && host.split('.').all(|label| !label.is_empty() && !label.starts_with('-') && !label.ends_with('-') && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
                if supported && valid_host {
                    if let Some(url) = web_url(&format!("https://{bare}")) {
                        links.push(WebLink { id: String::new(), domain: url.host_str().unwrap_or("").to_owned(), url: url.to_string() });
                    }
                }
            }
            for mut link in links {
                if !seen.insert(link.url.clone()) { continue; }
                if items.len() == 16 { limited = true; continue; }
                link.id = items.len().to_string();
                items.push(link);
            }
        }
    }
    Ok(ExtractedLinks { items, limited })
}

pub fn extract(text: &str) -> Result<ExtractedLinks, String> {
    if text.len() > 16 * 1024 || text.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')) {
        return Err("QR content cannot be extracted".into());
    }
    let lower = text.to_ascii_lowercase();
    // 整段就是一个地址时保留有意义的结尾标点；分号/逗号后紧跟协议则按多链接处理。
    if !text.trim().chars().any(text_delimiter) && ![",http://", ",https://", ";http://", ";https://"].iter().any(|separator| lower.contains(separator)) {
        if let Some(url) = web_url(text.trim()) {
            let domain = url.host_str().ok_or("QR invalid web URL")?.to_string();
            return Ok(ExtractedLinks { items: vec![WebLink { id: "0".into(), url: url.as_str().to_string(), domain }], limited: false });
        }
    }
    let mut cursor = 0;
    let mut items = Vec::new();
    let mut seen = HashSet::new();
    while cursor < text.len() {
        let Some(offset) = [lower[cursor..].find("http://"), lower[cursor..].find("https://")].into_iter().flatten().min() else { break };
        let start = cursor + offset;
        // 中文说明可以紧贴网址；ASCII 单词中的片段不能冒充独立协议。
        if text[..start].chars().next_back().is_some_and(|c| c.is_ascii_alphanumeric() || c == '_') {
            cursor = start + 4;
            continue;
        }
        let end = start + text[start..].char_indices().find(|(index, c)| {
            text_delimiter(*c)
                || matches!(c, ',' | ';') && ["http://", "https://"].iter().any(|scheme| lower[start+index+c.len_utf8()..].starts_with(scheme))
        }).map_or(text.len()-start, |(index, _)| index);
        let mut candidate = &text[start..end];
        cursor = end;
        let quoted = if text[..start].ends_with('\'') {
            candidate.trim_end_matches(['.', ',', ';', '!', '?', ':']).strip_suffix('\'')
        } else { None };
        if let Some(value) = quoted { candidate = value; }
        let mut round = 0i32;
        let mut square = 0i32;
        for c in candidate.chars() { match c { '(' => round += 1, ')' => round -= 1, '[' => square += 1, ']' => square -= 1, _ => {} } }
        // 去掉句尾标点和包围链接的括号，保留 URL 内平衡的括号及 IPv6 地址。
        while let Some(c) = candidate.chars().next_back().filter(|_| quoted.is_none()) {
            match c {
                '.' | ',' | ';' | '!' | '?' | ':' => {},
                ')' if round < 0 => round += 1,
                ']' if square < 0 => square += 1,
                _ => break,
            }
            candidate = &candidate[..candidate.len()-c.len_utf8()];
        }
        let Some(url) = web_url(candidate) else { continue };
        let normalized = url.as_str().to_string();
        if !seen.insert(normalized.clone()) { continue; }
        if items.len() == 16 { return Ok(ExtractedLinks { items, limited: true }); }
        let domain = url.host_str().ok_or("QR invalid web URL")?.to_string();
        items.push(WebLink { id: items.len().to_string(), url: normalized, domain });
    }
    Ok(ExtractedLinks { items, limited: false })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_web_links_from_chinese_text_and_deduplicates() {
        let result = extract("官网：https://example.com/中文?q=1。备用 https://other.example/a#part\n再次：https://example.com/中文?q=1").unwrap();
        assert!(!result.limited);
        assert_eq!(result.items.len(), 2);
        assert_eq!(result.items[0].domain, "example.com");
        assert_eq!(result.items[0].url, "https://example.com/%E4%B8%AD%E6%96%87?q=1");
        assert_eq!(result.items[1].url, "https://other.example/a#part");
        assert_eq!(result.items[0].id, "0");
    }

    #[test]
    fn preserves_url_parentheses_queries_and_removes_surrounding_punctuation() {
        let result = extract("[文档](https://example.com/wiki/QR_(code)). JSON: {\"url\":\"HTTP://EXAMPLE.COM:80/a?x=1&y=2\"}").unwrap();
        assert_eq!(result.items.iter().map(|link| link.url.as_str()).collect::<Vec<_>>(), ["https://example.com/wiki/QR_(code)", "http://example.com/a?x=1&y=2"]);
    }

    #[test]
    fn excludes_non_web_schemes_credentials_and_malformed_urls() {
        let result = extract("javascript:alert(1) file:///C:/Windows ftp://example.com https://user:password@example.com https:// https://[invalidhost] abcHTTPS://example.com").unwrap();
        assert!(result.items.is_empty());
        assert!(!result.limited);
    }

    #[test]
    fn returns_no_links_without_an_explicit_http_scheme() {
        assert!(extract("这是一段文本，www.example.com；扫码内容不含网页链接。").unwrap().items.is_empty());
        assert!(extract("").unwrap().items.is_empty());
    }

    #[test]
    fn deduplicates_canonical_addresses_and_preserves_encounter_order() {
        let result = extract("HTTPS://EXAMPLE.COM:443 https://example.com/ http://other.example/x").unwrap();
        assert_eq!(result.items.len(), 2);
        assert_eq!(result.items[0].url, "https://example.com/");
        assert_eq!(result.items[1].id, "1");
    }

    #[test]
    fn bounds_count_and_rejects_oversize_or_hidden_controls() {
        let text = (0..20).map(|n| format!("https://example.com/{n}")).collect::<Vec<_>>().join("\n");
        let result = extract(&text).unwrap();
        assert_eq!(result.items.len(), 16);
        assert!(result.limited);
        assert!(extract(&"x".repeat(16*1024+1)).is_err());
        assert!(extract("https://example.com\0hidden").is_err());
    }

    #[test]
    fn preserves_apostrophes_inside_links_and_handles_quoted_values() {
        let url = "https://en.wikipedia.org/wiki/Schindler's_List";
        assert_eq!(extract(url).unwrap().items[0].url, url);
        assert_eq!(extract(&format!("文档：'{url}'。")).unwrap().items[0].url, url);
        assert_eq!(extract("{'url':'https://example.com/a?query=it's'}").unwrap().items[0].url, "https://example.com/a?query=it%27s");
    }

    #[test]
    fn separates_links_on_punctuation_only_when_another_scheme_follows() {
        let result = extract("https://one.example/a,https://two.example/b;HTTP://three.example/c").unwrap();
        assert_eq!(result.items.iter().map(|link| link.domain.as_str()).collect::<Vec<_>>(), ["one.example", "two.example", "three.example"]);
        assert_eq!(extract("https://example.com/a,b;c?q=x,y;z").unwrap().items[0].url,"https://example.com/a,b;c?q=x,y;z");
    }

    #[test]
    fn preserves_significant_punctuation_when_the_entire_content_is_a_url() {
        for url in ["https://example.com/file.", "https://example.com/?q=last?", "https://example.com/#footer!"] {
            assert_eq!(extract(url).unwrap().items[0].url, url);
        }
        assert!(web_url("https://example.com/a https://other.example/b").is_none());
    }

    #[test]
    fn chinese_separators_are_not_swallowed_by_whole_url_handling() {
        for separator in ["，", "。", "；", "、"] {
            let text = format!("https://one.example/a{separator}https://two.example/b");
            let result = extract(&text).unwrap();
            assert_eq!(result.items.iter().map(|link| link.url.as_str()).collect::<Vec<_>>(), ["https://one.example/a", "https://two.example/b"]);
        }
        assert_eq!(extract("https://example.com/a。请查看").unwrap().items[0].url, "https://example.com/a");
    }

    #[test]
    fn screen_text_recognizes_visible_addresses_without_qr_content() {
        let result = extract_screen_text("屏幕上的网页\nhttps://example.com/path\nwww.example.org/docs\nexample.net/help\n报告.pdf\nnotes.txt\nuser@example.com").unwrap();
        assert_eq!(result.items.iter().map(|link| link.url.as_str()).collect::<Vec<_>>(), ["https://example.com/path", "https://www.example.org/docs", "https://example.net/help"]);
    }

    #[test]
    fn screen_text_handles_ocr_scheme_spacing_and_larger_bounded_text() {
        let text = format!("{}\nhttps : / /example.com/a\nHTTP: //other.example/b\nwww.example.org\n", "普通屏幕文字 ".repeat(1000));
        assert!(text.len() > 16*1024);
        let result = extract_screen_text(&text).unwrap();
        assert_eq!(result.items.len(), 3);
        assert_eq!(result.items[0].url, "https://example.com/a");
        assert!(extract_screen_text(&"x".repeat(65537)).is_err());
    }

    #[test]
    fn screen_text_limits_and_deduplicates_links_from_ocr_lines() {
        let text = (0..20).map(|i| format!("https://example.com/{i}\nhttps://example.com/{i}")).collect::<Vec<_>>().join("\n");
        let result = extract_screen_text(&text).unwrap();
        assert_eq!(result.items.len(), 16);
        assert!(result.limited);
        assert_eq!(result.items[15].id, "15");
    }

    #[test]
    fn screen_text_preserves_overflow_from_one_punctuation_separated_token() {
        let text = (0..17).map(|i| format!("https://example.com/{i}")).collect::<Vec<_>>().join(",");
        let result = extract_screen_text(&text).unwrap();
        assert_eq!(result.items.len(), 16);
        assert!(result.limited);
    }
}
