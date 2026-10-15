use serde::Deserialize;
use tauri::{PhysicalPosition, PhysicalRect, PhysicalSize};

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowLayout { Compact, Results, Settings }

pub struct Geometry {
    pub size: PhysicalSize<u32>,
    pub position: PhysicalPosition<i32>,
}

pub fn fit(layout: WindowLayout, rows: u32, width: f64, work: &PhysicalRect<i32, u32>, scale: f64, anchor: Option<PhysicalPosition<i32>>) -> Geometry {
    let height = match layout {
        WindowLayout::Compact => 76.0,
        WindowLayout::Results if rows == 0 => 76.0,
        WindowLayout::Results => 172.0 + 76.0 * rows.min(6) as f64,
        WindowLayout::Settings => 620.0,
    };
    let size = PhysicalSize::new(
        (width.max(360.0) * scale).round().clamp(1.0, work.size.width.max(1) as f64) as u32,
        (height * scale).round().clamp(1.0, work.size.height.max(1) as f64) as u32,
    );
    let left = work.position.x as f64;
    let top = work.position.y as f64;
    let right = left + work.size.width.saturating_sub(size.width) as f64;
    let bottom = top + work.size.height.saturating_sub(size.height) as f64;
    // 展开和再次呼出保留当前坐标；首次在工作区居中，所有坐标都限定于工作区。
    let (x,y) = anchor.map(|p| (p.x as f64, p.y as f64)).unwrap_or((left + (right-left)/2.0, top + (bottom-top)/2.0));
    Geometry { size, position: PhysicalPosition::new(x.clamp(left,right).round() as i32, y.clamp(top,bottom).round() as i32) }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn work() -> PhysicalRect<i32,u32> { PhysicalRect { position: PhysicalPosition::new(-1920,40), size: PhysicalSize::new(1920,1040) } }
    #[test]
    fn initial_window_centers_in_work_area_and_keeps_dragged_anchor() {
        let initial = fit(WindowLayout::Compact, 0, 680.0, &work(), 1.0, None);
        assert_eq!(initial.position, PhysicalPosition::new(-1300, 522));
        let dragged = PhysicalPosition::new(-1650, 370);
        assert_eq!(fit(WindowLayout::Compact, 0, 680.0, &work(), 1.0, Some(dragged)).position, dragged);
        assert_eq!(fit(WindowLayout::Results, 6, 680.0, &work(), 1.0, Some(dragged)).position, dragged);
    }
    #[test]
    fn layout_heights_follow_visible_rows_and_reject_unrecognized_contract() {
        for (layout, rows, height) in [(WindowLayout::Compact,6,76), (WindowLayout::Results,0,76), (WindowLayout::Results,1,248), (WindowLayout::Results,6,628), (WindowLayout::Results,u32::MAX,628), (WindowLayout::Settings,0,620)] {
            assert_eq!(fit(layout,rows,880.0,&work(),1.0,None).size.height,height);
        }
        assert!(serde_json::from_str::<WindowLayout>("\"unknown\"").is_err());
    }
    #[test]
    fn dpi_bounds_width_height_and_position_to_small_work_area() {
        let work = PhysicalRect { position: PhysicalPosition::new(1200,-500), size: PhysicalSize::new(800,400) };
        let result = fit(WindowLayout::Settings,0,880.0,&work,1.5,Some(PhysicalPosition::new(1990,-100)));
        assert_eq!(result.size, PhysicalSize::new(800,400));
        assert_eq!(result.position, work.position);
        assert_eq!(fit(WindowLayout::Compact,0,100.0,&work,1.5,None).size,PhysicalSize::new(540,114));
    }
    #[test]
    fn expansion_preserves_top_anchor_until_work_area_requires_clamping() {
        let anchor = PhysicalPosition::new(-1500,150);
        assert_eq!(fit(WindowLayout::Results,6,880.0,&work(),1.0,Some(anchor)).position,anchor);
        let anchor = PhysicalPosition::new(-100,1000);
        assert_eq!(fit(WindowLayout::Results,6,880.0,&work(),1.0,Some(anchor)).position,PhysicalPosition::new(-880,452));
    }
}
