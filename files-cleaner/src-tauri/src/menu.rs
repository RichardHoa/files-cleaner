//! Native menus on every OS. Clicks reach the UI as `menu` and `context-menu`
//! events.

use serde::Deserialize;
use tauri::menu::{IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, Window};

const CONTEXT_PREFIX: &str = "context:";

/// Ids must match `MENU_BAR_ITEMS` in menu-items.js.
const ITEMS: [(&str, &str); 10] = [
    ("newScan", "New Scan…"),
    ("selectAll", "Select All"),
    ("trash", "Trash"),
    ("trashNow", "Trash Now"),
    ("rename", "Rename"),
    ("review", "Review"),
    ("rules", "Rules"),
    ("history", "History"),
    ("settings", "Settings…"),
    ("help", "Introduction"),
];

/// Names must match `SHORTCUTS` in menu-items.js.
fn accelerator(name: &str) -> Option<&'static str> {
    let mac = cfg!(target_os = "macos");
    match name {
        "newScan" => Some("CmdOrCtrl+N"),
        "selectAll" => Some("CmdOrCtrl+A"),
        "trash" => Some(if mac { "Cmd+Backspace" } else { "Ctrl+Delete" }),
        "trashNow" => Some(if mac {
            "Shift+Cmd+Backspace"
        } else {
            "Ctrl+Shift+Delete"
        }),
        "rename" => Some("Enter"),
        "settings" => Some("CmdOrCtrl+Comma"),
        _ => None,
    }
}

/// `text` shown as it is: an `&` would otherwise mark a mnemonic.
fn literal(text: &str) -> String {
    text.replace('&', "&&")
}

pub struct MenuBar {
    items: Vec<MenuItem<tauri::Wry>>,
    recent: Submenu<tauri::Wry>,
    move_to: Submenu<tauri::Wry>,
    merge_into: Submenu<tauri::Wry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuState {
    enabled: Vec<String>,
    recent: Vec<String>,
    move_to: Vec<String>,
    merge_into: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextItem {
    label: String,
    shortcut: Option<String>,
    enabled: bool,
    submenu: Option<Vec<ContextItem>>,
    separator: bool,
}

/// Greyed out until the UI says what applies.
pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let items = ITEMS
        .iter()
        .map(|(id, label)| {
            // Windows may take a disabled item's key from the window, so Return
            // is only written there, never bound.
            if *id == "rename" && cfg!(windows) {
                return MenuItem::with_id(app, *id, format!("{label}\tEnter"), false, None::<&str>);
            }
            MenuItem::with_id(app, *id, label, false, accelerator(id))
        })
        .collect::<tauri::Result<Vec<_>>>()?;
    let item = |id: &str| &items[ITEMS.iter().position(|(item, _)| *item == id).unwrap()];
    let separator = || PredefinedMenuItem::separator(app);

    let recent = Submenu::with_id(app, "recent", "Scan Recent", false)?;
    let move_to = Submenu::with_id(app, "moveTo", "Move To…", false)?;
    let merge_into = Submenu::with_id(app, "mergeInto", "Merge Into…", false)?;

    let file = Submenu::with_items(app, "File", true, &[item("newScan"), &recent])?;
    #[cfg(not(target_os = "macos"))]
    file.append_items(&[&separator()?, &PredefinedMenuItem::quit(app, None)?])?;
    // On macOS, Cut, Copy and Paste in text fields only work through the menu
    // bar.
    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &separator()?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            item("selectAll"),
            &separator()?,
            item("trash"),
            item("trashNow"),
            item("rename"),
            &merge_into,
            &move_to,
        ],
    )?;
    let view = Submenu::with_items(
        app,
        "View",
        true,
        &[
            item("review"),
            item("rules"),
            item("history"),
            &separator()?,
            item("settings"),
        ],
    )?;
    let window = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, None)?,
            &separator()?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;
    let help = Submenu::with_items(app, "Help", true, &[item("help")])?;

    let menu = Menu::new(app)?;
    #[cfg(target_os = "macos")]
    {
        let name = app.package_info().name.clone();
        let app_menu = Submenu::with_items(
            app,
            &name,
            true,
            &[
                &PredefinedMenuItem::about(app, None, None)?,
                &separator()?,
                &PredefinedMenuItem::services(app, None)?,
                &separator()?,
                &PredefinedMenuItem::hide(app, None)?,
                &PredefinedMenuItem::hide_others(app, None)?,
                &PredefinedMenuItem::show_all(app, None)?,
                &separator()?,
                &PredefinedMenuItem::quit(app, None)?,
            ],
        )?;
        menu.append(&app_menu)?;
        window.set_as_windows_menu_for_nsapp()?;
        help.set_as_help_menu_for_nsapp()?;
    }
    menu.append_items(&[&file, &edit, &view, &window, &help])?;
    app.set_menu(menu)?;
    app.on_menu_event(send_click);
    app.manage(MenuBar {
        items,
        recent,
        move_to,
        merge_into,
    });
    Ok(())
}

fn send_click(app: &AppHandle, event: MenuEvent) {
    let id = event.id().as_ref();
    let _ = match id.strip_prefix(CONTEXT_PREFIX) {
        Some(place) => app.emit("context-menu", place),
        None => app.emit("menu", id),
    };
}

#[tauri::command]
pub fn set_menu_state(
    app: AppHandle,
    bar: tauri::State<'_, MenuBar>,
    state: MenuState,
) -> tauri::Result<()> {
    for (item, (id, _)) in bar.items.iter().zip(ITEMS) {
        item.set_enabled(state.enabled.iter().any(|enabled| enabled == id))?;
    }
    fill(&app, &bar.recent, "recent", &state.recent)?;
    fill(&app, &bar.move_to, "moveTo", &state.move_to)?;
    fill(&app, &bar.merge_into, "mergeInto", &state.merge_into)
}

fn fill(
    app: &AppHandle,
    submenu: &Submenu<tauri::Wry>,
    list: &str,
    labels: &[String],
) -> tauri::Result<()> {
    for old in submenu.items()? {
        submenu.remove(&old)?;
    }
    for (index, label) in labels.iter().enumerate() {
        submenu.append(&MenuItem::with_id(
            app,
            format!("{list}:{index}"),
            literal(label),
            true,
            None::<&str>,
        )?)?;
    }
    submenu.set_enabled(!labels.is_empty())
}

#[tauri::command]
pub fn show_context_menu(window: Window, items: Vec<ContextItem>) -> tauri::Result<()> {
    let menu = Menu::new(&window)?;
    for item in context_items(&window, &items, "")? {
        menu.append(item.as_ref())?;
    }
    window.popup_menu(&menu)
}

/// Ids are places in the menu, "1.0" for a submenu's first, so the UI can find
/// the action again.
fn context_items(
    window: &Window,
    items: &[ContextItem],
    parent: &str,
) -> tauri::Result<Vec<Box<dyn IsMenuItem<tauri::Wry>>>> {
    items
        .iter()
        .enumerate()
        .map(
            |(index, item)| -> tauri::Result<Box<dyn IsMenuItem<tauri::Wry>>> {
                let place = if parent.is_empty() {
                    index.to_string()
                } else {
                    format!("{parent}.{index}")
                };
                if item.separator {
                    return Ok(Box::new(PredefinedMenuItem::separator(window)?));
                }
                if let Some(children) = &item.submenu {
                    let submenu = Submenu::new(window, literal(&item.label), item.enabled)?;
                    for child in context_items(window, children, &place)? {
                        submenu.append(child.as_ref())?;
                    }
                    return Ok(Box::new(submenu));
                }
                let shortcut = item.shortcut.as_deref().and_then(accelerator);
                Ok(Box::new(MenuItem::with_id(
                    window,
                    format!("{CONTEXT_PREFIX}{place}"),
                    literal(&item.label),
                    item.enabled,
                    shortcut,
                )?))
            },
        )
        .collect()
}
