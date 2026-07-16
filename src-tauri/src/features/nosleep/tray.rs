use tauri::{
    include_image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle,
};

const TRAY_ID: &str = "nosleep-status";

fn tray_menu(app: &AppHandle, active_guards: &[String]) -> tauri::Result<Menu<tauri::Wry>> {
    let menu = Menu::new(app)?;

    if active_guards.is_empty() {
        let empty_item =
            MenuItem::with_id(app, "guard-empty", "No active guards", false, None::<&str>)?;
        menu.append(&empty_item)?;
        return Ok(menu);
    }

    let title_item = MenuItem::with_id(app, "guard-title", "Active guards", false, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    menu.append(&title_item)?;
    menu.append(&separator)?;

    for (index, guard) in active_guards.iter().enumerate() {
        let item = MenuItem::with_id(
            app,
            format!("guard-active-{index}"),
            guard,
            false,
            None::<&str>,
        )?;
        menu.append(&item)?;
    }

    Ok(menu)
}

fn update_icon(app: &AppHandle, has_active_guards: bool) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let icon = if has_active_guards {
            include_image!("./icons/nosleep-on.png")
        } else {
            include_image!("./icons/nosleep-off.png")
        };

        if let Err(error) = tray.set_icon_with_as_template(Some(icon), true) {
            eprintln!("failed to update nosleep tray icon: {error}");
        }
    }
}

pub(crate) fn update_nosleep_tray(app: &AppHandle, active_guards: &[String]) {
    update_icon(app, !active_guards.is_empty());

    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };

    match tray_menu(app, active_guards) {
        Ok(menu) => {
            if let Err(error) = tray.set_menu(Some(menu)) {
                eprintln!("failed to update nosleep tray menu: {error}");
            }
        }
        Err(error) => eprintln!("failed to build nosleep tray menu: {error}"),
    }
}

pub(crate) fn create_nosleep_tray(app: &AppHandle) -> tauri::Result<()> {
    if app.tray_by_id(TRAY_ID).is_some() {
        return Ok(());
    }

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(include_image!("./icons/nosleep-off.png"))
        .icon_as_template(true)
        .menu(&tray_menu(app, &[])?)
        .build(app)?;
    Ok(())
}

pub(crate) fn remove_nosleep_tray(app: &AppHandle) {
    app.remove_tray_by_id(TRAY_ID);
}
