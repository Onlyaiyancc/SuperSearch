use image::{DynamicImage, GrayImage};
use std::time::{Duration, Instant};
use windows::{
    core::w,
    Win32::{Foundation::HWND, Graphics::Gdi::*, UI::WindowsAndMessaging::*},
};

struct FixtureWindow(HWND);
impl Drop for FixtureWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.0);
        }
    }
}
struct FixtureDc {
    window: HWND,
    dc: HDC,
}
impl Drop for FixtureDc {
    fn drop(&mut self) {
        unsafe {
            let _ = ReleaseDC(Some(self.window), self.dc);
        }
    }
}

#[cfg(test)]
mod image_tests {
    #[test]
    fn synthetic_image_contains_uncropped_qr_and_three_visible_text_lines() {
        let image = super::screen_links_image_fixture().unwrap();
        let qr = crate::qr_scan::decode(&image).unwrap().0;
        assert!(qr
            .iter()
            .any(|item| item.text == "https://example.invalid/supersearch-qr-smoke"));
        let code = zxingcpp::create(zxingcpp::BarcodeFormat::QRCode)
            .from_str("https://example.invalid/supersearch-qr-smoke")
            .unwrap();
        let qr_image = code
            .to_image_with(&zxingcpp::write().scale(8).add_quiet_zones(true))
            .unwrap();
        assert_eq!(image.height(), qr_image.height() as u32 + 180);
        for top in [18, 60, 102] {
            assert!((top..top + 34).any(|y| (30..900).any(|x| image.get_pixel(x, y)[0] < 128)));
        }
    }
}

pub fn run() -> Result<(), String> {
    run_fixture(false)
}

pub fn run_screen_links() -> Result<(), String> {
    run_fixture(true)
}

struct ImageFixtureResources {
    dc: HDC,
    bitmap: HBITMAP,
    font: HFONT,
    previous_bitmap: HGDIOBJ,
    previous_font: HGDIOBJ,
}

impl Drop for ImageFixtureResources {
    fn drop(&mut self) {
        unsafe {
            if !self.previous_font.is_invalid() {
                let _ = SelectObject(self.dc, self.previous_font);
            }
            if !self.previous_bitmap.is_invalid() {
                let _ = SelectObject(self.dc, self.previous_bitmap);
            }
            if !self.font.is_invalid() {
                let _ = DeleteObject(self.font.into());
            }
            if !self.bitmap.is_invalid() {
                let _ = DeleteObject(self.bitmap.into());
            }
            if !self.dc.is_invalid() {
                let _ = DeleteDC(self.dc);
            }
        }
    }
}

fn screen_links_image_fixture() -> Result<image::RgbaImage, String> {
    let code = zxingcpp::create(zxingcpp::BarcodeFormat::QRCode)
        .from_str("https://example.invalid/supersearch-qr-smoke")
        .map_err(|_| "QR fixture generation failed")?;
    let output = code
        .to_image_with(&zxingcpp::write().scale(8).add_quiet_zones(true))
        .map_err(|_| "QR fixture generation failed")?;
    let gray = GrayImage::from_raw(output.width() as u32, output.height() as u32, output.data())
        .ok_or("QR fixture generation failed")?;
    let qr = DynamicImage::ImageLuma8(gray).to_rgba8();
    let width = 1000;
    let height = qr
        .height()
        .checked_add(180)
        .ok_or("Screen URL fixture size unavailable")?;
    let bytes = crate::qr_capture::checked_bytes(width, height)?;
    unsafe {
        // 纯内存 DC 和 DIB 只承载已知合成内容，不获取桌面 DC，也不读取或保存用户屏幕。
        let mut resources = ImageFixtureResources {
            dc: CreateCompatibleDC(None),
            bitmap: HBITMAP::default(),
            font: HFONT::default(),
            previous_bitmap: HGDIOBJ::default(),
            previous_font: HGDIOBJ::default(),
        };
        if resources.dc.is_invalid() {
            return Err("Screen URL fixture canvas unavailable".into());
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        resources.bitmap = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0)
            .map_err(|_| "Screen URL fixture canvas unavailable")?;
        if bits.is_null() {
            return Err("Screen URL fixture canvas unavailable".into());
        }
        resources.previous_bitmap = SelectObject(resources.dc, resources.bitmap.into());
        if resources.previous_bitmap.is_invalid() {
            return Err("Screen URL fixture canvas unavailable".into());
        }
        PatBlt(resources.dc, 0, 0, width as i32, height as i32, WHITENESS)
            .ok()
            .map_err(|_| "Screen URL fixture drawing failed")?;
        resources.font = CreateFontW(
            28,
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            NONANTIALIASED_QUALITY,
            DEFAULT_PITCH.0 as u32,
            w!("Segoe UI"),
        );
        if resources.font.is_invalid() {
            return Err("Screen URL fixture font unavailable".into());
        }
        resources.previous_font = SelectObject(resources.dc, resources.font.into());
        if resources.previous_font.is_invalid() {
            return Err("Screen URL fixture font unavailable".into());
        }
        SetTextColor(resources.dc, windows::Win32::Foundation::COLORREF(0));
        SetBkColor(resources.dc, windows::Win32::Foundation::COLORREF(0xFFFFFF));
        for (row, line) in [
            "https://example.com/screen-one",
            "www.example.org/screen-two",
            "example.net/help",
        ]
        .iter()
        .enumerate()
        {
            let text: Vec<u16> = line.encode_utf16().collect();
            TextOutW(resources.dc, 30, 18 + row as i32 * 42, &text)
                .ok()
                .map_err(|_| "Screen URL fixture drawing failed")?;
        }
        GdiFlush()
            .ok()
            .map_err(|_| "Screen URL fixture drawing failed")?;
        let mut rgba = std::slice::from_raw_parts(bits.cast::<u8>(), bytes).to_vec();
        for pixel in rgba.chunks_exact_mut(4) {
            pixel.swap(0, 2);
            pixel[3] = 255;
        }
        let mut canvas = image::RgbaImage::from_raw(width, height, rgba)
            .ok_or("Screen URL fixture canvas unavailable")?;
        // 底部保留完整二维码静区，文字网址与二维码载荷保持独立。
        image::imageops::overlay(&mut canvas, &qr, 30, 160);
        Ok(canvas)
    }
}

pub fn run_screen_links_image() -> Result<(), String> {
    let started = Instant::now();
    let root = std::env::current_dir()
        .map_err(|_| "Screen URL diagnostic storage unavailable")?
        .join("test-results/screen-links");
    std::fs::create_dir_all(&root).map_err(|_| "Screen URL diagnostic storage unavailable")?;
    let expected = [
        "https://example.com/screen-one",
        "https://www.example.org/screen-two",
        "https://example.net/help",
    ];
    let mut actual_ocr_engine = false;
    let mut qr_detected = false;
    let mut fixture_links = Vec::new();
    let mut recognized_links = Vec::new();
    let mut dimensions = (0, 0);
    let outcome = (|| -> Result<(), String> {
        let image = screen_links_image_fixture()?;
        // 仅保存本函数创建的已知合成画布，绝不保存真实桌面捕获。
        image
            .save(root.join("synthetic-ocr-fixture.png"))
            .map_err(|_| "Screen URL synthetic fixture storage unavailable")?;
        dimensions = image.dimensions();
        let (items, _) = crate::qr_scan::decode(&image)?;
        qr_detected = items
            .iter()
            .any(|item| item.text == "https://example.invalid/supersearch-qr-smoke");
        if !qr_detected {
            return Err("QR synthetic image fixture was not detected".into());
        }
        let executable =
            std::env::current_exe().map_err(|_| "Screen URL diagnostic executable unavailable")?;
        // 使用相同的组件校验、安装租约、内存映射和实际 worker，不以模拟识别结果替代引擎。
        let links = crate::conversion_components::recognize_screen_links_at(
            &root.join("native-image-runtime"),
            &executable,
            &image,
            &std::sync::atomic::AtomicBool::new(false),
        )?;
        actual_ocr_engine = true;
        recognized_links = links.items.clone();
        fixture_links = expected
            .iter()
            .copied()
            .filter(|url| links.items.iter().any(|link| link.url.as_str() == *url))
            .collect();
        if fixture_links.len() != expected.len() {
            return Err("Screen URL fixture addresses were not all recognized".into());
        }
        Ok(())
    })();
    // 报告只列出预先写死的合成网址；该诊断不宣称验证了真实屏幕捕获或多屏交互。
    let report = serde_json::json!({
        "status":if outcome.is_ok() {"passed"} else {"failed"},
        "check":"real-ocr-synthetic-image", "durationMs":started.elapsed().as_millis(),
        "width":dimensions.0, "height":dimensions.1,
        "expectedWebLinks":expected, "recognizedFixtureLinks":fixture_links,
        "recognizedLinks":recognized_links, "syntheticImageSaved":true,
        "qrDetected":qr_detected, "actualOcrEngine":actual_ocr_engine,
        "screenCaptureVerified":false, "physicalMultiMonitorVerified":false,
        "screenshotSaved":false, "ocrTextSaved":false, "error":outcome.as_ref().err(),
    });
    let json = serde_json::to_string_pretty(&report)
        .map_err(|_| "Screen URL diagnostic report unavailable")?;
    std::fs::write(root.join("native-image.json"), &json)
        .map_err(|_| "Screen URL diagnostic storage unavailable")?;
    println!("{json}");
    outcome
}

fn run_fixture(screen_links: bool) -> Result<(), String> {
    let started = Instant::now();
    let text = "https://example.invalid/supersearch-qr-smoke";
    let code = zxingcpp::create(zxingcpp::BarcodeFormat::QRCode)
        .from_str(text)
        .map_err(|_| "QR fixture generation failed")?;
    let output = code
        .to_image_with(&zxingcpp::write().scale(8).add_quiet_zones(true))
        .map_err(|_| "QR fixture generation failed")?;
    let image = GrayImage::from_raw(output.width() as u32, output.height() as u32, output.data())
        .ok_or("QR fixture generation failed")?;
    let qr = DynamicImage::ImageLuma8(image).to_rgba8();
    let pixels = if screen_links {
        let mut canvas = image::RgbaImage::from_pixel(1000, 540, image::Rgba([255, 255, 255, 255]));
        image::imageops::overlay(&mut canvas, &qr, 30, 160);
        canvas
    } else {
        qr
    };
    let rect = crate::qr_capture::cursor_monitor()?;
    if pixels.width() > rect.width || pixels.height() > rect.height {
        return Err("QR fixture display too small".into());
    }
    unsafe {
        // 诊断只临时展示合成二维码，不抢焦点、不访问用户存储，也不保存屏幕图像。
        let fixture = FixtureWindow(
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                w!("STATIC"),
                w!("SuperSearch QR diagnostic"),
                WS_POPUP | WS_VISIBLE,
                rect.left + ((rect.width - pixels.width()) / 2) as i32,
                rect.top + ((rect.height - pixels.height()) / 2) as i32,
                pixels.width() as i32,
                pixels.height() as i32,
                None,
                None,
                None,
                None,
            )
            .map_err(|_| "QR fixture window unavailable")?,
        );
        let dc = FixtureDc {
            window: fixture.0,
            dc: GetDC(Some(fixture.0)),
        };
        if dc.dc.is_invalid() {
            return Err("QR fixture window unavailable".into());
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: pixels.width() as i32,
                biHeight: -(pixels.height() as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let _ = UpdateWindow(fixture.0);
        for _ in 0..3 {
            let lines = SetDIBitsToDevice(
                dc.dc,
                0,
                0,
                pixels.width(),
                pixels.height(),
                0,
                0,
                0,
                pixels.height(),
                pixels.as_ptr().cast(),
                &info,
                DIB_RGB_COLORS,
            );
            if lines == 0 {
                return Err("QR fixture drawing failed".into());
            }
            if screen_links {
                // 合成文字与二维码同时显示；测试网址并未写进二维码载荷。
                let font = CreateFontW(
                    28,
                    0,
                    0,
                    0,
                    FW_NORMAL.0 as i32,
                    0,
                    0,
                    0,
                    DEFAULT_CHARSET,
                    OUT_DEFAULT_PRECIS,
                    CLIP_DEFAULT_PRECIS,
                    NONANTIALIASED_QUALITY,
                    DEFAULT_PITCH.0 as u32,
                    w!("Segoe UI"),
                );
                if font.is_invalid() {
                    return Err("Screen URL fixture font unavailable".into());
                }
                let old = SelectObject(dc.dc, font.into());
                SetTextColor(dc.dc, windows::Win32::Foundation::COLORREF(0));
                SetBkColor(dc.dc, windows::Win32::Foundation::COLORREF(0xFFFFFF));
                for (row, line) in [
                    "https://example.com/screen-one",
                    "www.example.org/screen-two",
                    "example.net/help",
                ]
                .iter()
                .enumerate()
                {
                    let wide: Vec<u16> = line.encode_utf16().collect();
                    if !TextOutW(dc.dc, 30, 18 + row as i32 * 42, &wide).as_bool() {
                        return Err("Screen URL fixture drawing failed".into());
                    }
                }
                SelectObject(dc.dc, old);
                let _ = DeleteObject(font.into());
            }
            let _ = GdiFlush();
            std::thread::sleep(Duration::from_millis(160));
            let captured = crate::qr_capture::capture(rect)?;
            let (items, _) = crate::qr_scan::decode(&captured)?;
            if items.iter().any(|item| item.text == text) {
                if screen_links {
                    let root = std::env::current_dir()
                        .map_err(|_| "Screen URL diagnostic storage unavailable")?
                        .join("test-results/screen-links");
                    std::fs::create_dir_all(&root)
                        .map_err(|_| "Screen URL diagnostic storage unavailable")?;
                    let runtime = root.join(format!("native-runtime-{}", std::process::id()));
                    let executable = std::env::current_exe()
                        .map_err(|_| "Screen URL diagnostic executable unavailable")?;
                    let links = crate::conversion_components::recognize_screen_links_at(
                        &runtime,
                        &executable,
                        &captured,
                        &std::sync::atomic::AtomicBool::new(false),
                    )?;
                    let expected = [
                        "https://example.com/screen-one",
                        "https://www.example.org/screen-two",
                        "https://example.net/help",
                    ];
                    let passed = expected
                        .iter()
                        .all(|url| links.items.iter().any(|link| &link.url == url));
                    let fixture_links: Vec<_> = expected
                        .iter()
                        .copied()
                        .filter(|url| links.items.iter().any(|link| link.url.as_str() == *url))
                        .collect();
                    let report = serde_json::json!({"status":if passed {"passed"} else {"failed"},"check":"real-screen-web-links-and-qr","durationMs":started.elapsed().as_millis(),"expectedWebLinks":expected,"recognizedFixtureLinks":fixture_links,"qrDetected":true,"screenshotSaved":false,"ocrTextSaved":false,"physicalMultiMonitorVerified":false});
                    let json = serde_json::to_string_pretty(&report)
                        .map_err(|_| "Screen URL diagnostic report unavailable")?;
                    std::fs::write(root.join("native-screen.json"), &json)
                        .map_err(|_| "Screen URL diagnostic storage unavailable")?;
                    println!("{json}");
                    return if passed {
                        Ok(())
                    } else {
                        Err("Screen URL fixture addresses were not all recognized".into())
                    };
                }
                println!(
                    "{}",
                    serde_json::json!({"status":"passed","check":"real-screen-qr-fixture","width":rect.width,"height":rect.height,"durationMs":started.elapsed().as_millis(),"screenshotSaved":false,"physicalMultiMonitorVerified":false})
                );
                return Ok(());
            }
        }
    }
    Err("QR fixture was not detected on the current desktop".into())
}
