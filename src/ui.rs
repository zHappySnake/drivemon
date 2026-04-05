use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, List, ListItem, ListState, Paragraph, Row, Sparkline,
        Table, Wrap,
    },
};

use crate::app::App;
use crate::disk::{SmartStatus, format_bytes, format_speed};

// ---- Entry point ----

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    draw_header(f, app, chunks[0]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(22), Constraint::Min(0)])
        .split(chunks[1]);

    draw_disk_list(f, app, body[0]);
    draw_detail(f, app, body[1]);
}

// ---- Header / tab bar ----

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let tab_labels = [
        (" Overview ", app.tab == 0),
        (" SMART Attrs ", app.tab == 1),
        (" I/O History ", app.tab == 2),
    ];

    let mut spans: Vec<Span> = vec![Span::styled(
        " 💽 drivemon  ",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )];

    for (label, active) in &tab_labels {
        let style = if *active {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        spans.push(Span::styled(*label, style));
        spans.push(Span::raw("  "));
    }

    spans.push(Span::styled(
        "  [↑↓/jk] disk  [Tab] tab  [r] refresh SMART  [q] quit",
        Style::default().fg(Color::DarkGray),
    ));

    let header = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(header, area);
}

// ---- Disk list (left panel) ----

fn draw_disk_list(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .disks
        .iter()
        .enumerate()
        .map(|(i, disk)| {
            let selected = i == app.selected;

            let dot = match disk.smart_status {
                SmartStatus::Passed => Span::styled("● ", Style::default().fg(Color::Green)),
                SmartStatus::Failed => Span::styled("● ", Style::default().fg(Color::Red)),
                SmartStatus::Unknown => Span::styled("○ ", Style::default().fg(Color::DarkGray)),
            };

            let name_style = if selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let name = Span::styled(format!("/dev/{}", disk.name), name_style);

            let temp = disk
                .temperature
                .map(|t| {
                    Span::styled(
                        format!(" {}°C", t as i64),
                        Style::default().fg(temp_color(t)),
                    )
                })
                .unwrap_or_else(|| Span::raw(""));

            ListItem::new(Line::from(vec![dot, name, temp]))
        })
        .collect();

    let mut state = ListState::default();
    state.select(Some(app.selected));

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(Span::styled(
                    " Drives ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .highlight_style(Style::default().bg(Color::Rgb(30, 30, 50)));

    f.render_stateful_widget(list, area, &mut state);
}

// ---- Right panel dispatcher ----

fn draw_detail(f: &mut Frame, app: &App, area: Rect) {
    let Some(disk) = app.selected_disk() else {
        let p = Paragraph::new("No drives detected.\nIs /sys/block accessible?")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .alignment(Alignment::Center);
        f.render_widget(p, area);
        return;
    };

    match app.tab {
        0 => draw_overview(f, disk, area),
        1 => draw_smart_attrs(f, disk, area),
        2 => draw_io_history(f, disk, area),
        _ => {}
    }
}

// ---- Tab 0: Overview ----

fn draw_overview(f: &mut Frame, disk: &crate::disk::DiskInfo, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // info card
            Constraint::Length(4), // read sparkline
            Constraint::Length(4), // write sparkline
            Constraint::Min(3),    // partitions
        ])
        .split(area);

    draw_info_card(f, disk, chunks[0]);
    draw_sparkline(f, disk, chunks[1], false);
    draw_sparkline(f, disk, chunks[2], true);
    draw_partitions(f, disk, chunks[3]);
}

fn draw_info_card(f: &mut Frame, disk: &crate::disk::DiskInfo, area: Rect) {
    let model_line = if disk.model.is_empty() {
        format!("/dev/{}", disk.name)
    } else {
        format!("/dev/{}  ─  {}", disk.name, disk.model)
    };

    let status_str = match disk.smart_status {
        SmartStatus::Passed => "✔ PASSED",
        SmartStatus::Failed => "✘ FAILED",
        SmartStatus::Unknown => "? Unknown",
    };
    let status_color = match disk.smart_status {
        SmartStatus::Passed => Color::Green,
        SmartStatus::Failed => Color::Red,
        SmartStatus::Unknown => Color::Yellow,
    };

    let temp_str = disk
        .temperature
        .map(|t| format!("  🌡 {}°C", t as i64))
        .unwrap_or_default();

    let age_str = disk
        .power_on_hours
        .map(|h| {
            let years = h as f64 / 8760.0;
            format!("  ⏱ {h} h ({years:.1} yr)")
        })
        .unwrap_or_default();

    let cap_str = if disk.capacity_bytes > 0 {
        format!("  💾 {}", format_bytes(disk.capacity_bytes))
    } else {
        String::new()
    };

    let serial_str = if disk.serial.is_empty() {
        String::new()
    } else {
        format!("  S/N: {}", disk.serial)
    };

    let note_str = disk
        .smart_note
        .as_deref()
        .map(|n| format!("  ⚠ {n}"))
        .unwrap_or_default();

    let text: Vec<Line> = vec![
        Line::from(Span::styled(
            model_line,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled(
                status_str,
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(temp_str, Style::default().fg(Color::Yellow)),
            Span::styled(age_str, Style::default().fg(Color::Gray)),
            Span::styled(cap_str, Style::default().fg(Color::Gray)),
        ]),
        Line::from(vec![
            Span::styled(serial_str, Style::default().fg(Color::DarkGray)),
            Span::styled(note_str, Style::default().fg(Color::Yellow)),
        ]),
    ];

    let card = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(Span::styled(
                    " Drive Info ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(card, area);
}

fn draw_sparkline(f: &mut Frame, disk: &crate::disk::DiskInfo, area: Rect, is_write: bool) {
    let (history, speed_bps, color, label) = if is_write {
        (
            &disk.write_history,
            disk.write_speed_bps,
            Color::Blue,
            " Write ",
        )
    } else {
        (
            &disk.read_history,
            disk.read_speed_bps,
            Color::Green,
            " Read  ",
        )
    };

    let data: Vec<u64> = history.iter().map(|&v| v as u64).collect();
    let max_val = data.iter().copied().max().unwrap_or(1).max(1);

    let title_right = format!(" {} ", format_speed(speed_bps));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color))
        .title(Span::styled(
            label,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ))
        .title(Span::styled(
            title_right,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));

    let spark = Sparkline::default()
        .block(block)
        .data(&data)
        .max(max_val)
        .style(Style::default().fg(color));

    f.render_widget(spark, area);
}

fn draw_partitions(f: &mut Frame, disk: &crate::disk::DiskInfo, area: Rect) {
    if disk.partitions.is_empty() {
        let p = Paragraph::new("No mounted partitions found for this drive.").block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(Span::styled(
                    " Partitions ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
        );
        f.render_widget(p, area);
        return;
    }

    let inner_w = area.width.saturating_sub(4) as usize; // minus borders + padding
    let bar_w = (inner_w.saturating_sub(38)).max(10);

    let rows: Vec<Row> = disk
        .partitions
        .iter()
        .map(|p| {
            let is_mounted = !p.mount_point.is_empty() && p.mount_point != "[SWAP]";
            let is_swap = p.mount_point == "[SWAP]";

            let (_ratio, pct, bar, bar_style) =
                if is_mounted && p.total_bytes > 0 && p.free_bytes > 0 {
                    let ratio = p.usage_ratio();
                    let pct = ratio * 100.0;
                    let bar = bar_fill(ratio, bar_w);
                    let bar_style = match pct as u64 {
                        0..=69 => Style::default().fg(Color::Green),
                        70..=89 => Style::default().fg(Color::Yellow),
                        _ => Style::default().fg(Color::Red),
                    };
                    (ratio, pct, bar, bar_style)
                } else if is_swap {
                    // Swap partition - show full bar in blue
                    let bar = bar_fill(1.0, bar_w);
                    let bar_style = Style::default().fg(Color::Blue);
                    (1.0, 100.0, bar, bar_style)
                } else {
                    // Unmounted or encrypted partition - show empty bar
                    let bar = bar_fill(0.0, bar_w);
                    let bar_style = Style::default().fg(Color::DarkGray);
                    (0.0, 0.0, bar, bar_style)
                };

            let dev_short = p
                .device
                .strip_prefix("/dev/")
                .unwrap_or(&p.device)
                .to_string();

            let mount_display = if p.mount_point.is_empty() {
                "[not mounted]".to_string()
            } else if p.mount_point == "[SWAP]" {
                "[SWAP]".to_string()
            } else {
                p.mount_point.clone()
            };

            let pct_display = if is_mounted {
                format!("{pct:.0}%")
            } else if is_swap {
                "100%".to_string()
            } else {
                "N/A".to_string()
            };

            let capacity_display = if is_mounted {
                format!(
                    "{} / {} free",
                    format_bytes(p.total_bytes),
                    format_bytes(p.free_bytes)
                )
            } else if is_swap {
                format!("{} swap", format_bytes(p.total_bytes))
            } else if p.total_bytes > 0 {
                format!("{} total", format_bytes(p.total_bytes))
            } else {
                "N/A".to_string()
            };

            Row::new(vec![
                Cell::from(dev_short).style(Style::default().fg(Color::White)),
                Cell::from(mount_display).style(Style::default().fg(Color::Gray)),
                Cell::from(p.fs_type.clone()).style(Style::default().fg(Color::DarkGray)),
                Cell::from(bar).style(bar_style),
                Cell::from(pct_display).style(Style::default().fg(Color::White)),
                Cell::from(capacity_display).style(Style::default().fg(Color::Gray)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(10),
        Constraint::Length(14),
        Constraint::Length(6),
        Constraint::Min(10),
        Constraint::Length(5),
        Constraint::Length(22),
    ];

    let table = Table::new(rows, widths)
        .header(
            Row::new(vec!["Device", "Mount", "FS", "Usage", "%", "Capacity"])
                .style(
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                )
                .bottom_margin(0),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(Span::styled(
                    " Partitions ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .column_spacing(1);

    f.render_widget(table, area);
}

// ---- Tab 1: SMART Attributes ----

fn draw_smart_attrs(f: &mut Frame, disk: &crate::disk::DiskInfo, area: Rect) {
    if disk.smart_attrs.is_empty() {
        let msg = disk
            .smart_note
            .as_deref()
            .unwrap_or("No SMART attributes available. Try running as root.");
        let p = Paragraph::new(format!("\n  {msg}\n\n  Press [r] to refresh SMART data.")).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(Span::styled(
                    " SMART Attributes ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
        );
        f.render_widget(p, area);
        return;
    }

    let rows: Vec<Row> = disk
        .smart_attrs
        .iter()
        .map(|a| {
            // Flag if value is close to or below threshold
            let warn = a.thresh > 0 && a.value <= a.thresh + 5;
            let name_style = if warn {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            Row::new(vec![
                Cell::from(if a.id == 0 {
                    "   ".into()
                } else {
                    format!("{:>3}", a.id)
                })
                .style(Style::default().fg(Color::DarkGray)),
                Cell::from(a.name.clone()).style(name_style),
                Cell::from(format!("{:>5}", a.value)).style(Style::default().fg(Color::Gray)),
                Cell::from(format!("{:>5}", a.worst)).style(Style::default().fg(Color::DarkGray)),
                Cell::from(format!("{:>5}", a.thresh)).style(if a.thresh > 0 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::DarkGray)
                }),
                Cell::from(a.raw_string.clone()).style(Style::default().fg(Color::Cyan)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Min(28),
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Length(7),
        Constraint::Min(16),
    ];

    let table = Table::new(rows, widths)
        .header(
            Row::new(vec!["ID", "Attribute", "Value", "Worst", "Thresh", "Raw"])
                .style(
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                )
                .bottom_margin(0),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(Span::styled(
                    " SMART Attributes ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ))
                .title(Span::styled(
                    " [r] refresh ",
                    Style::default().fg(Color::DarkGray),
                )),
        )
        .column_spacing(1);

    f.render_widget(table, area);
}

// ---- Tab 2: I/O History ----

fn draw_io_history(f: &mut Frame, disk: &crate::disk::DiskInfo, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    draw_large_sparkline(f, disk, chunks[0], false);
    draw_large_sparkline(f, disk, chunks[1], true);
}

fn draw_large_sparkline(f: &mut Frame, disk: &crate::disk::DiskInfo, area: Rect, is_write: bool) {
    let (history, speed_bps, color, label) = if is_write {
        (
            &disk.write_history,
            disk.write_speed_bps,
            Color::Blue,
            " Write Speed  (60 s) ",
        )
    } else {
        (
            &disk.read_history,
            disk.read_speed_bps,
            Color::Green,
            " Read Speed  (60 s) ",
        )
    };

    let data: Vec<u64> = history.iter().map(|&v| v as u64).collect();
    let max_val = data.iter().copied().max().unwrap_or(1).max(1);

    let right_title = format!(
        " now: {}  peak: {} ",
        format_speed(speed_bps),
        format_speed(max_val as f64)
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color))
        .title(Span::styled(
            label,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ))
        .title(Span::styled(right_title, Style::default().fg(Color::White)));

    let spark = Sparkline::default()
        .block(block)
        .data(&data)
        .max(max_val)
        .style(Style::default().fg(color));

    f.render_widget(spark, area);
}

// ---- Colour helpers ----

fn temp_color(celsius: f64) -> Color {
    match celsius as i64 {
        i64::MIN..=39 => Color::Green,
        40..=54 => Color::Yellow,
        _ => Color::Red,
    }
}

// ---- Bar helper ----

fn bar_fill(ratio: f64, width: usize) -> String {
    let filled = ((ratio * width as f64).round() as usize).min(width);
    let empty = width.saturating_sub(filled);
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}
