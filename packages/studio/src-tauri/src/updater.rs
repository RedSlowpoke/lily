//! Updates (DECISIONS D124): Tauri's updater asks
//! https://lily.everyday.fail/dist/latest.json (`plugins.updater` in
//! tauri.conf.json) a few seconds after launch and when Lily Studio › Check for
//! Updates… is chosen. A newer version is installed only when the user says
//! so, and the studio relaunches into it once its edits are saved or
//! dropped. Only a release build checks: the updater replaces the bundle the
//! program runs from, and `npm start`'s program is in no bundle. The page
//! never reaches the plugin.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;

use lily_session::Studio;

use crate::dialogs;

/// Lily Studio › Check for Updates….
pub const CHECK: &str = "check-for-updates";
/// How long after launch the first check waits, so the window is up before an alert.
pub const AT_LAUNCH: Duration = Duration::from_secs(3);

/// Whether a check or an installation is under way; another is not started meanwhile.
#[derive(Default)]
pub struct Checking(AtomicBool);

/// Checks for an update and offers it. `asked` when the menu asked: then
/// "up to date" and a failed check are said too, not only an update found.
pub async fn check(app: AppHandle, asked: bool) {
    if app.state::<Checking>().0.swap(true, Ordering::SeqCst) {
        return;
    }
    let failed = match offer(&app, asked).await {
        Ok(()) => None,
        Err(Failure::Check(error)) if !asked => {
            eprintln!("Check for Updates: {error}");
            None
        }
        Err(Failure::Check(error)) => Some(("Lily Studio could not check for updates.", error)),
        Err(Failure::Install(error)) => Some(("Lily Studio could not be updated.", error)),
    };
    app.state::<Checking>().0.store(false, Ordering::SeqCst);
    if let Some((message, detail)) = failed {
        let _ = dialogs::alert(
            &app,
            dialogs::Alert {
                message: message.into(),
                detail,
                buttons: vec!["OK".into()],
            },
        )
        .await;
    }
}

enum Failure {
    Check(String),
    Install(String),
}

async fn offer(app: &AppHandle, asked: bool) -> Result<(), Failure> {
    let update = app
        .updater()
        .map_err(|error| Failure::Check(error.to_string()))?
        .check()
        .await
        .map_err(|error| Failure::Check(error.to_string()))?;
    let Some(update) = update else {
        if asked {
            let _ = dialogs::alert(
                app,
                dialogs::Alert {
                    message: "Lily Studio is up to date.".into(),
                    detail: format!("Version {} is the newest.", app.package_info().version),
                    buttons: vec!["OK".into()],
                },
            )
            .await;
        }
        return Ok(());
    };
    let mut detail = format!("You have {}.", update.current_version);
    if let Some(notes) = update
        .body
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
    {
        detail = format!("{detail}\n\n{notes}");
    }
    let answer = dialogs::alert(
        app,
        dialogs::Alert {
            message: format!("Lily Studio {} is available.", update.version),
            detail,
            buttons: vec!["Install and Relaunch".into(), "Later".into()],
        },
    )
    .await;
    if answer != Ok(0) {
        return Ok(());
    }
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|error| Failure::Install(error.to_string()))?;
    // Installed: a Cancel here leaves the new version for the next launch.
    let studio = app.state::<Arc<Studio>>().inner().clone();
    if studio.dirty.load(Ordering::SeqCst)
        && let Some(window) = app.get_webview_window(crate::WINDOW)
        && !crate::edits_settled(&window, &studio).await
    {
        return Ok(());
    }
    // RunEvent::Exit stops what runs and cleans up, as quitting does.
    app.request_restart();
    Ok(())
}
