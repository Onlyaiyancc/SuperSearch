use serde::Deserialize;
use tauri::{PhysicalPosition, PhysicalRect, PhysicalSize};

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowLayout { Compact, Results, Settings }

pub struct Geometry {
    pub size: PhysicalSize<u32>,
    pub position: PhysicalPosition<i32>,
}

#[derive(Default)]
pub struct MonitorMemory { monitor: Option<PhysicalRect<i32,u32>> }

impl MonitorMemory {
    fn disconnected(&self, connected: &[PhysicalRect<i32,u32>]) -> bool {
        !connected.is_empty() && self.monitor.as_ref().is_some_and(|previous| {
            !connected.iter().any(|monitor| monitor.position == previous.position && monitor.size == previous.size)
        })
    }
    pub fn anchor(&self, connected: &[PhysicalRect<i32,u32>], position: Option<PhysicalPosition<i32>>) -> Option<PhysicalPosition<i32>> {
        if self.disconnected(connected) { None } else { position }
    }
    pub fn observe(&mut self, monitor: PhysicalRect<i32,u32>, connected: &[PhysicalRect<i32,u32>]) {
        // 旧屏消失后保留失效标记，不能把 Windows 自动搬移当成用户重新选择位置。
        if !connected.is_empty() && !self.disconnected(connected) { self.placed(monitor); }
    }
    pub fn placed(&mut self, monitor: PhysicalRect<i32,u32>) { self.monitor = Some(monitor); }
}

pub fn fit(layout: WindowLayout, rows: u32, work: &PhysicalRect<i32, u32>, scale: f64, anchor: Option<PhysicalPosition<i32>>) -> Geometry {
    let height = match layout {
        WindowLayout::Compact => 76.0,
        WindowLayout::Results if rows == 0 => 76.0,
        WindowLayout::Results => 172.0 + 76.0 * rows.min(6) as f64,
        WindowLayout::Settings => 620.0,
    };
    let size = PhysicalSize::new(
        // 始终从产品逻辑宽度计算，断屏导致的临时收缩不能成为下一次的正常宽度。
        (680.0 * scale).round().clamp(1.0, work.size.width.max(1) as f64) as u32,
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
    fn primary() -> PhysicalRect<i32,u32> { PhysicalRect { position: PhysicalPosition::new(0,0), size: PhysicalSize::new(1920,1080) } }
    #[test]
    fn disconnected_monitor_centers_even_after_windows_moves_the_window_to_a_corner() {
        let mut memory = MonitorMemory::default();
        memory.observe(work(), &[work(),primary()]);
        // Windows 可能先把窗口搬到主屏左上角；该移动不能冒充用户的新位置。
        memory.observe(primary(), &[primary()]);
        for position in [PhysicalPosition::new(-1500,250), PhysicalPosition::new(0,0)] {
            let anchor = memory.anchor(&[primary()], Some(position));
            assert_eq!(fit(WindowLayout::Compact,0,&primary(),1.0,anchor).position,PhysicalPosition::new(620,502));
        }
        memory.placed(primary());
        let moved = PhysicalPosition::new(350,275);
        assert_eq!(memory.anchor(&[primary()],Some(moved)),Some(moved));
    }
    #[test]
    fn ordinary_cross_display_dragging_updates_the_remembered_monitor() {
        let mut memory = MonitorMemory::default();
        memory.observe(primary(), &[work(),primary()]);
        memory.observe(work(), &[work(),primary()]);
        let moved = PhysicalPosition::new(-1500,250);
        assert_eq!(memory.anchor(&[work(),primary()],Some(moved)),Some(moved));
        assert_eq!(memory.anchor(&[primary()],Some(PhysicalPosition::new(0,0))),None);
    }
    #[test]
    fn removing_an_unrelated_display_preserves_a_valid_remembered_corner() {
        let mut memory = MonitorMemory::default();
        memory.observe(primary(), &[work(),primary()]);
        let corner = PhysicalPosition::new(0,0);
        assert_eq!(memory.anchor(&[primary()],Some(corner)),Some(corner));
        // 拓扑查询暂时为空时不破坏记忆，稍后可重试。
        assert_eq!(memory.anchor(&[],Some(corner)),Some(corner));
    }
    #[test]
    fn unplugging_a_small_monitor_restores_canonical_width() {
        let small = PhysicalRect { position: PhysicalPosition::new(0,0), size: PhysicalSize::new(440,800) };
        let shrunk = fit(WindowLayout::Compact,0,&small,1.0,None);
        assert_eq!(shrunk.size.width,440);
        let restored = fit(WindowLayout::Compact,0,&work(),1.0,None);
        assert_eq!(restored.size.width,680);
    }
    fn work() -> PhysicalRect<i32,u32> { PhysicalRect { position: PhysicalPosition::new(-1920,40), size: PhysicalSize::new(1920,1040) } }
    #[test]
    fn initial_window_centers_in_work_area_and_keeps_dragged_anchor() {
        let initial = fit(WindowLayout::Compact, 0, &work(), 1.0, None);
        assert_eq!(initial.position, PhysicalPosition::new(-1300, 522));
        let dragged = PhysicalPosition::new(-1650, 370);
        assert_eq!(fit(WindowLayout::Compact, 0, &work(), 1.0, Some(dragged)).position, dragged);
        assert_eq!(fit(WindowLayout::Results, 6, &work(), 1.0, Some(dragged)).position, dragged);
    }
    #[test]
    fn layout_heights_follow_visible_rows_and_reject_unrecognized_contract() {
        for (layout, rows, height) in [(WindowLayout::Compact,6,76), (WindowLayout::Results,0,76), (WindowLayout::Results,1,248), (WindowLayout::Results,6,628), (WindowLayout::Results,u32::MAX,628), (WindowLayout::Settings,0,620)] {
            assert_eq!(fit(layout,rows,&work(),1.0,None).size.height,height);
        }
        assert!(serde_json::from_str::<WindowLayout>("\"unknown\"").is_err());
    }
    #[test]
    fn dpi_bounds_width_height_and_position_to_small_work_area() {
        let small_work = PhysicalRect { position: PhysicalPosition::new(1200,-500), size: PhysicalSize::new(800,400) };
        let result = fit(WindowLayout::Settings,0,&small_work,1.5,Some(PhysicalPosition::new(1990,-100)));
        assert_eq!(result.size, PhysicalSize::new(800,400));
        assert_eq!(result.position, small_work.position);
        assert_eq!(fit(WindowLayout::Compact,0,&small_work,1.5,None).size,PhysicalSize::new(800,114));
        assert_eq!(fit(WindowLayout::Compact,0,&work(),2.0,None).size,PhysicalSize::new(1360,152));
    }
    #[test]
    fn expansion_preserves_top_anchor_until_work_area_requires_clamping() {
        let anchor = PhysicalPosition::new(-1500,150);
        assert_eq!(fit(WindowLayout::Results,6,&work(),1.0,Some(anchor)).position,anchor);
        let anchor = PhysicalPosition::new(-100,1000);
        assert_eq!(fit(WindowLayout::Results,6,&work(),1.0,Some(anchor)).position,PhysicalPosition::new(-680,452));
    }
}
