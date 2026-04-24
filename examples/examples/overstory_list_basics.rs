// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Overstory list surface basics.
//!
//! Build a small retained list surface, then drive it through row clicks and
//! keyboard navigation to show how `overstory_list` owns focus/selection state
//! above the raw Overstory element tree.
//!
//! Run:
//! - `cargo run -p understory_examples --example overstory_list_basics`

use kurbo::Rect;
use overstory::ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};
use overstory::{Color, Panel, ScrollView, TextBlock, Ui, default_theme};
use overstory_list::{ListRowPresentation, ListViewController, ListViewStyle};

fn main() {
    let mut ui = Ui::new(default_theme());
    ui.set_view_rect(Rect::new(0.0, 0.0, 320.0, 280.0));
    ui.set_local(ui.root(), ui.properties().padding, 16.0);
    ui.set_local(ui.root(), ui.properties().gap, 12.0);

    let shell = ui.append(
        ui.root(),
        Panel::new()
            .padding(14.0)
            .gap(10.0)
            .background(Color::from_rgba8(249, 247, 242, 255)),
    );
    let title = ui.append(
        shell,
        TextBlock::new()
            .with_text("Projects")
            .font_size(14.0)
            .label_padding(0.0)
            .padding(0.0),
    );
    let _ = title;

    let scroll = ui.append(shell, ScrollView::new().height(180.0).padding(0.0).gap(0.0));
    let mut list = ListViewController::<&'static str>::new(scroll);
    list.set_style(ListViewStyle {
        row_padding: 2.0,
        row_corner_radius: 8.0,
        font_size: 12.0,
        label_padding: 8.0,
        background: Color::TRANSPARENT,
        selected_background: Color::from_rgba8(55, 107, 86, 255),
        focused_background: Color::from_rgba8(226, 222, 213, 255),
    });

    let rows = [
        ListRowPresentation::new("alpha", "Alpha"),
        ListRowPresentation::new("bravo", "Bravo"),
        ListRowPresentation::new("charlie", "Charlie"),
        ListRowPresentation::new("delta", "Delta"),
    ];
    list.sync(&mut ui, &rows);

    println!("== Initial ==");
    print_list_state(&mut ui, &list);

    let bravo_ids = list.realized_rows()[1].ids;
    let click = list.handle_click(bravo_ids.label);
    list.sync(&mut ui, &rows);
    println!("\n== Click Bravo ==");
    println!("action: {click:?}");
    print_list_state(&mut ui, &list);

    let down = list.handle_keyboard_event(&KeyboardEvent::key_down(
        Key::Named(NamedKey::ArrowDown),
        Code::ArrowDown,
    ));
    list.sync(&mut ui, &rows);
    println!("\n== Arrow Down ==");
    println!("action: {down:?}");
    print_list_state(&mut ui, &list);

    let end = list.handle_keyboard_event(&KeyboardEvent::key_down(
        Key::Named(NamedKey::End),
        Code::End,
    ));
    list.sync(&mut ui, &rows);
    println!("\n== End ==");
    println!("action: {end:?}");
    print_list_state(&mut ui, &list);

    let activate = list.handle_keyboard_event(&KeyboardEvent::key_down(
        Key::Named(NamedKey::Enter),
        Code::Enter,
    ));
    list.sync(&mut ui, &rows);
    println!("\n== Enter ==");
    println!("action: {activate:?}");
    print_list_state(&mut ui, &list);
}

fn print_list_state(ui: &mut Ui, list: &ListViewController<&'static str>) {
    println!(
        "focused={:?} selected={:?}",
        list.focused_key(),
        list.selected_key()
    );
    let scene = ui.scene();
    for row in list.realized_rows() {
        let resolved = scene
            .resolved_element(row.ids.row)
            .expect("list row should be visible");
        let text = scene
            .resolved_element(row.ids.label)
            .and_then(|element| element.text.as_deref())
            .unwrap_or("-");
        let bg = resolved.background.to_rgba8();
        println!(
            "row {:?}: bg=rgba({:02x},{:02x},{:02x},{:02x}) focused={} text={text}",
            row.key, bg.r, bg.g, bg.b, bg.a, row.focused,
        );
    }
}
