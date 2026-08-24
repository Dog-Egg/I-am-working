use std::sync::{Arc, Mutex};

use tauri::AppHandle;

use crate::app::state::AppState;
use crate::features::nosleep::inhibitor::sync_sleep_inhibitor;
use crate::features::nosleep::state::{adjust_guard_count, sorted_active_guards};
use crate::features::nosleep::tray::update_nosleep_tray;

pub(crate) enum GuardUpdateError {
    Disabled,
    StateUnavailable,
}

pub(crate) fn update_guard(
    app: &AppHandle,
    state: &Arc<Mutex<AppState>>,
    name: &str,
    active: bool,
) -> Result<(), GuardUpdateError> {
    let mut state = state
        .lock()
        .map_err(|_| GuardUpdateError::StateUnavailable)?;
    if !state.settings.settings.nosleep_enabled {
        if active {
            return Err(GuardUpdateError::Disabled);
        }
        // `off` is idempotent cleanup, even when the feature is disabled.
        return Ok(());
    }

    adjust_guard_count(&mut state.nosleep.active_guards, name, active);
    let active_guards = sorted_active_guards(&state.nosleep.active_guards);
    update_nosleep_tray(app, &active_guards);
    sync_sleep_inhibitor(!active_guards.is_empty());

    Ok(())
}

pub(crate) fn clear_guard(
    app: &AppHandle,
    state: &Arc<Mutex<AppState>>,
    name: &str,
) -> Result<(), GuardUpdateError> {
    let mut state = state
        .lock()
        .map_err(|_| GuardUpdateError::StateUnavailable)?;
    if !state.settings.settings.nosleep_enabled {
        return Ok(());
    }

    state.nosleep.active_guards.remove(name.trim());
    let active_guards = sorted_active_guards(&state.nosleep.active_guards);
    update_nosleep_tray(app, &active_guards);
    sync_sleep_inhibitor(!active_guards.is_empty());

    Ok(())
}
