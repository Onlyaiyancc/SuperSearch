use std::{fs, path::Path};
use windows::{core::{HSTRING, PCWSTR}, Data::Xml::Dom::XmlDocument, UI::Notifications::{ToastNotification, ToastNotificationManager}, Win32::System::Registry::{RegCloseKey, RegCreateKeyW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, REG_SZ}};

const APP_ID: &str = "com.supersearch.desktop";
const APP_NAME: &str = "SuperSearch";
const APP_ICON: &[u8] = include_bytes!("../icons/icon.png");

struct RegistryKey(HKEY);
impl Drop for RegistryKey {
    fn drop(&mut self) { unsafe { let _ = RegCloseKey(self.0); } }
}

fn registry_text(key: &RegistryKey, name: &str, value: &str) -> Result<(), String> {
    let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    let value: Vec<u16> = value.encode_utf16().chain(Some(0)).collect();
    // 注册表字符串包括 UTF-16 终止符，借用仅在同步 API 调用期间有效。
    let bytes = unsafe { std::slice::from_raw_parts(value.as_ptr().cast::<u8>(), value.len() * 2) };
    unsafe { RegSetValueExW(key.0, PCWSTR(name.as_ptr()), None, REG_SZ, Some(bytes)).ok() }
        .map_err(|_| "Unable to register notification identity".into())
}

fn register_identity(directory: &Path) -> Result<(), String> {
    let folder = directory.join("notifications");
    fs::create_dir_all(&folder).map_err(|_| "Unable to prepare notification icon")?;
    let icon = folder.join("app-icon.png");
    if fs::read(&icon).ok().as_deref() != Some(APP_ICON) {
        fs::write(&icon, APP_ICON).map_err(|_| "Unable to prepare notification icon")?;
    }
    // 明确注册当前用户的应用身份，安装版和直接运行版均不借用 PowerShell。
    let path: Vec<u16> = format!(r"Software\Classes\AppUserModelId\{APP_ID}").encode_utf16().chain(Some(0)).collect();
    let mut handle = HKEY::default();
    unsafe { RegCreateKeyW(HKEY_CURRENT_USER, PCWSTR(path.as_ptr()), &mut handle).ok() }
        .map_err(|_| "Unable to register notification identity")?;
    let key = RegistryKey(handle);
    registry_text(&key, "DisplayName", APP_NAME)?;
    registry_text(&key, "IconUri", &icon.to_string_lossy())
}

fn notification_xml(body: &str) -> windows::core::Result<XmlDocument> {
    let xml = XmlDocument::new()?;
    xml.LoadXml(&HSTRING::from(r#"<toast><visual><binding template="ToastGeneric"><text/></binding></visual></toast>"#))?;
    // 正文通过文本节点设置，提醒中的尖括号等字符不会成为 XML 标记。
    xml.GetElementsByTagName(&HSTRING::from("text"))?.Item(0)?.SetInnerText(&HSTRING::from(body))?;
    Ok(xml)
}

pub fn show(directory: &Path, body: &str) -> Result<(), String> {
    register_identity(directory)?;
    let xml = notification_xml(body).map_err(|_| "Unable to prepare notification")?;
    let toast = ToastNotification::CreateToastNotification(&xml).map_err(|_| "Unable to prepare notification")?;
    let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_ID)).map_err(|_| "Unable to prepare notification")?;
    notifier.Show(&toast).map_err(|_| "Unable to send notification".into())
}

#[cfg(test)]
mod tests {
    #[test]
    fn notification_identity_matches_application_configuration() {
        let config: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(config["identifier"], super::APP_ID);
        assert_eq!(config["productName"], super::APP_NAME);
    }

    #[test]
    fn notification_content_is_one_plain_text_block_without_duplicate_app_name() {
        let body = "休息 <script>test</script> & 喝水";
        let xml = super::notification_xml(body).unwrap();
        let blocks = xml.GetElementsByTagName(&windows::core::HSTRING::from("text")).unwrap();
        assert_eq!(blocks.Length().unwrap(), 1);
        assert_eq!(blocks.Item(0).unwrap().InnerText().unwrap().to_string(), body);
        assert_eq!(xml.GetElementsByTagName(&windows::core::HSTRING::from("script")).unwrap().Length().unwrap(), 0);
        assert!(!xml.GetXml().unwrap().to_string().contains("SuperSearch 倒计时提醒"));
    }

    #[test]
    #[ignore = "Sends one live Windows notification and registers this application's current-user identity"]
    fn live_notification_identity_probe() {
        let directory = std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA").expect("Local application data path")).join(super::APP_ID);
        super::show(&directory, "通知样式测试：应用名称与图标已更新").expect("Send branded notification");
        println!("Branded notification accepted by Windows: {}", super::APP_ID);
    }
}
