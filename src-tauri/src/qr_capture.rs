use image::RgbaImage;
use windows::Win32::{
    Foundation::POINT,
    Graphics::Gdi::*,
    UI::WindowsAndMessaging::GetCursorPos,
};

#[derive(Clone, Copy)]
pub struct CaptureRect {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
}

pub fn checked_bytes(width: u32, height: u32) -> Result<usize, String> {
    let pixels = u64::from(width).checked_mul(u64::from(height)).ok_or("QR screen too large")?;
    if pixels == 0 || pixels > 40_000_000 || width > i32::MAX as u32 || height > i32::MAX as u32 {
        return Err("QR screen too large".into());
    }
    usize::try_from(pixels * 4).map_err(|_| "QR screen too large".into())
}

pub fn cursor_monitor() -> Result<CaptureRect, String> {
    unsafe {
        let mut point = POINT::default();
        GetCursorPos(&mut point).map_err(|_| "QR screen capture unavailable")?;
        let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        GetMonitorInfoW(monitor, &mut info).ok().map_err(|_| "QR screen capture unavailable")?;
        let width = info.rcMonitor.right.checked_sub(info.rcMonitor.left).and_then(|n| u32::try_from(n).ok()).ok_or("QR screen too large")?;
        let height = info.rcMonitor.bottom.checked_sub(info.rcMonitor.top).and_then(|n| u32::try_from(n).ok()).ok_or("QR screen too large")?;
        checked_bytes(width, height)?;
        Ok(CaptureRect { left: info.rcMonitor.left, top: info.rcMonitor.top, width, height })
    }
}

struct CaptureResources { screen: HDC, memory: HDC, bitmap: HBITMAP, previous: HGDIOBJ }

impl Drop for CaptureResources {
    fn drop(&mut self) {
        // 先恢复原先选入的对象，再销毁位图和 DC；所有失败路径也遵守同样的释放顺序。
        unsafe {
            if !self.previous.is_invalid() { let _ = SelectObject(self.memory, self.previous); }
            if !self.bitmap.is_invalid() { let _ = DeleteObject(self.bitmap.into()); }
            if !self.memory.is_invalid() { let _ = DeleteDC(self.memory); }
            if !self.screen.is_invalid() { let _ = ReleaseDC(None, self.screen); }
        }
    }
}

pub fn capture(rect: CaptureRect) -> Result<RgbaImage, String> {
    let bytes = checked_bytes(rect.width, rect.height)?;
    unsafe {
        let mut resources = CaptureResources { screen: GetDC(None), memory: HDC::default(), bitmap: HBITMAP::default(), previous: HGDIOBJ::default() };
        if resources.screen.is_invalid() { return Err("QR screen capture unavailable".into()); }
        resources.memory = CreateCompatibleDC(Some(resources.screen));
        if resources.memory.is_invalid() { return Err("QR screen capture unavailable".into()); }
        let info = BITMAPINFO { bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: rect.width as i32, biHeight: -(rect.height as i32),
            biPlanes: 1, biBitCount: 32, biCompression: BI_RGB.0,
            ..Default::default()
        }, ..Default::default() };
        let mut bits = std::ptr::null_mut();
        resources.bitmap = CreateDIBSection(Some(resources.screen), &info, DIB_RGB_COLORS, &mut bits, None, 0).map_err(|_| "QR screen capture unavailable")?;
        if bits.is_null() { return Err("QR screen capture unavailable".into()); }
        resources.previous = SelectObject(resources.memory, resources.bitmap.into());
        if resources.previous.is_invalid() { return Err("QR screen capture unavailable".into()); }
        BitBlt(resources.memory, 0, 0, rect.width as i32, rect.height as i32, Some(resources.screen), rect.left, rect.top, SRCCOPY | CAPTUREBLT).map_err(|_| "QR screen capture unavailable")?;
        GdiFlush().ok().map_err(|_| "QR screen capture unavailable")?;
        let mut pixels = std::slice::from_raw_parts(bits.cast::<u8>(), bytes).to_vec();
        // Windows DIB 为 BGRA；补齐不透明 alpha 后，缩略图和解码共同使用现有 image 类型。
        for pixel in pixels.chunks_exact_mut(4) { pixel.swap(0, 2); pixel[3] = 255; }
        RgbaImage::from_raw(rect.width, rect.height, pixels).ok_or_else(|| "QR screen capture unavailable".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qr_capture_bounds_check_before_allocation() {
        assert_eq!(checked_bytes(3840, 2160).unwrap(), 33_177_600);
        assert!(checked_bytes(0, 100).is_err());
        assert!(checked_bytes(100, 0).is_err());
        assert!(checked_bytes(u32::MAX, u32::MAX).is_err());
        assert!(checked_bytes(40_000_001, 1).is_err());
        assert_eq!(checked_bytes(8000, 5000).unwrap(), 160_000_000);
    }
}
