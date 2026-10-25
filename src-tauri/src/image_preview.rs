use windows::{core::{Error, Interface}, Win32::{
    Foundation::E_INVALIDARG,
    Graphics::Imaging::{CLSID_WICImagingFactory, GUID_ContainerFormatPng, GUID_WICPixelFormat32bppBGRA, IWICBitmapFrameEncode, IWICBitmapSource, IWICImagingFactory, WICBitmapDitherTypeNone, WICBitmapEncoderNoCache, WICBitmapInterpolationModeFant, WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnDemand},
    System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, STATFLAG_NONAME, STATSTG, STREAM_SEEK_SET},
    UI::Shell::SHCreateMemStream,
}};

pub(super) const OUTPUT_LIMIT: usize = 16 * 1024 * 1024;
pub(super) const LONG_EDGE: u32 = 2048;

struct Apartment;
impl Drop for Apartment { fn drop(&mut self) { unsafe { CoUninitialize(); } } }

pub(super) fn thumbnail(bytes: &[u8]) -> Result<Vec<u8>, String> {
    // WIC 内置 TIFF/ICO 解码器；仅使用已校验文件的内存，不重新按路径打开。
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok().map_err(|_| "Preview image unavailable")?; }
    let _apartment = Apartment;
    let result = (|| -> windows::core::Result<Vec<u8>> {
        unsafe {
            let factory: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
            let input = factory.CreateStream()?;
            input.InitializeFromMemory(bytes)?;
            let decoder = factory.CreateDecoderFromStream(&input, std::ptr::null(), WICDecodeMetadataCacheOnDemand)?;
            let frame = decoder.GetFrame(0)?;
            let (mut width, mut height) = (0, 0);
            frame.GetSize(&mut width, &mut height)?;
            if width == 0 || height == 0 || u64::from(width) * u64::from(height) > super::PIXEL_LIMIT {
                return Err(Error::from_hresult(E_INVALIDARG));
            }
            let edge = width.max(height);
            let (width, height) = if edge > LONG_EDGE {
                (((u64::from(width) * u64::from(LONG_EDGE)) / u64::from(edge)).max(1) as u32,
                 ((u64::from(height) * u64::from(LONG_EDGE)) / u64::from(edge)).max(1) as u32)
            } else { (width, height) };
            let scaler = factory.CreateBitmapScaler()?;
            scaler.Initialize(&frame, width, height, WICBitmapInterpolationModeFant)?;
            let converter = factory.CreateFormatConverter()?;
            converter.Initialize(&scaler, &GUID_WICPixelFormat32bppBGRA, WICBitmapDitherTypeNone, None, 0.0, WICBitmapPaletteTypeCustom)?;
            let output = SHCreateMemStream(None).ok_or_else(Error::from_win32)?;
            let encoder = factory.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())?;
            encoder.Initialize(&output, WICBitmapEncoderNoCache)?;
            let mut encoded: Option<IWICBitmapFrameEncode> = None;
            encoder.CreateNewFrame(&mut encoded, std::ptr::null_mut())?;
            let encoded = encoded.ok_or_else(Error::from_win32)?;
            encoded.Initialize(None)?;
            encoded.SetSize(width, height)?;
            let mut format = GUID_WICPixelFormat32bppBGRA;
            encoded.SetPixelFormat(&mut format)?;
            // 缩放后的像素最多 2048²；编码流增长受此像素边界约束。
            let source: IWICBitmapSource = converter.cast()?;
            encoded.WriteSource(&source, std::ptr::null())?;
            encoded.Commit()?;
            encoder.Commit()?;
            let mut stat = STATSTG::default();
            output.Stat(&mut stat, STATFLAG_NONAME)?;
            if stat.cbSize == 0 || stat.cbSize > OUTPUT_LIMIT as u64 {
                return Err(Error::from_hresult(E_INVALIDARG));
            }
            let mut png = vec![0; stat.cbSize as usize];
            output.Seek(0, STREAM_SEEK_SET, None)?;
            let mut count = 0;
            output.Read(png.as_mut_ptr().cast(), png.len() as u32, Some(&mut count)).ok()?;
            if count as usize != png.len() { return Err(Error::from_hresult(E_INVALIDARG)); }
            Ok(png)
        }
    })();
    result.map_err(|_| "Preview invalid image".into())
}
