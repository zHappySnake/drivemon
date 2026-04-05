use crate::disk::{DiskCollector, DiskInfo};

pub struct App {
    pub disks: Vec<DiskInfo>,
    pub selected: usize,
    /// 0 = Overview, 1 = SMART Attributes, 2 = I/O History
    pub tab: usize,
    collector: DiskCollector,
}

impl App {
    pub fn new() -> Self {
        let mut collector = DiskCollector::new();
        let disks = collector.collect();
        App {
            selected: 0,
            tab: 0,
            disks,
            collector,
        }
    }

    /// Called every tick (~1 s): refreshes I/O stats but not SMART.
    pub fn on_tick(&mut self) {
        self.disks = self.collector.collect();
    }

    /// Triggered by the user pressing 'r': re-queries smartctl for all drives.
    pub fn refresh_smart(&mut self) {
        self.collector.refresh_smart();
        self.disks = self.collector.collect();
    }

    pub fn next_disk(&mut self) {
        if !self.disks.is_empty() {
            self.selected = (self.selected + 1) % self.disks.len();
        }
    }

    pub fn prev_disk(&mut self) {
        if !self.disks.is_empty() && self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn next_tab(&mut self) {
        self.tab = (self.tab + 1) % 3;
    }

    pub fn prev_tab(&mut self) {
        self.tab = if self.tab == 0 { 2 } else { self.tab - 1 };
    }

    pub fn selected_disk(&self) -> Option<&DiskInfo> {
        self.disks.get(self.selected)
    }
}
