//! File: strip/commands.rs
//! Purpose: The strip page's commands, one per user action, each checked against the open session so a late or foreign call changes nothing: the page is ready for the open event, place the window, run a row, cancel that run, paste a row's result, close.
//! Contents: strip_ready; strip_place; strip_run; strip_cancel; strip_paste; strip_close.

use tauri::AppHandle;

use super::bounds::Dip;
use super::{dismiss, send, state, window, LAST};
use crate::{palette, preview, settings};

/// The page has registered for the open event: it is sent now if the text is already copied, else as soon as it is.
#[tauri::command]
pub fn strip_ready(app: AppHandle) {
    let mut s = state();
    if s.page_ready() {
        send(&app, &mut s);
    }
}

/// The page measured its card for the view it now shows and asks for the window `rect` (dip, relative to the work area). The first call shows the window and gives it the keyboard; the timing of both is logged. A window that cannot take the keyboard is closed, since it could neither be used nor clicked away. Input: the session and the rectangle. Return: Err for a request that is not the open session's or not a sane rectangle, or when the window could not be shown.
#[tauri::command]
pub fn strip_place(app: AppHandle, session: u64, rect: Dip) -> Result<(), String> {
    let found = state().place(session, rect);
    let Some((physical, first, pressed)) = found else { return Err("This strip is closed or the size is not valid.".into()) };
    if first {
        crate::log::note(&format!("strip: first paint reported at {} ms", pressed.elapsed().as_millis()));
    }
    if !window::place(&app, physical, first) {
        dismiss(&app);
        return Err("The strip could not be shown.".into());
    }
    if first {
        crate::log::note(&format!("strip: visible at {} ms", pressed.elapsed().as_millis()));
    }
    Ok(())
}

/// Runs row `id` as request `seq` of the session. A row with an exact result answers with it at once (the open event leaves results out for a long text); a text-model row runs, and a newer request, a cancel or a close makes it give up; its result is kept so Paste delivers exactly it. Input: the session, the row id and the request number. Return: the result, or None when it changes nothing, failed (already reported) or is no longer wanted.
#[tauri::command]
pub async fn strip_run(session: u64, id: String, seq: u64) -> Option<String> {
    let (text, program, epoch) = {
        let mut s = state();
        let found = s.session(session).filter(|x| x.has_row(&id))?;
        if let Some(exact) = found.exact(&id) {
            return Some(exact);
        }
        (found.text.clone(), found.program.clone(), found.epoch)
    };
    let row = id.clone();
    let out = tauri::async_runtime::spawn_blocking(move || palette::compute(&row, &text, &program, epoch, seq)).await.ok().flatten()?;
    state().session(session)?.keep(id, out.clone());
    Some(out)
}

/// The user stepped back from a running text-model row: it stops, and later runs belong to a new palette session. The page waits for this before it starts another run. Input: the session.
#[tauri::command]
pub fn strip_cancel(session: u64) {
    let mut s = state();
    if s.session(session).is_some() {
        palette::cancel();
        s.renew(session, palette::open());
    }
}

/// Pastes row `id`'s result into the field the text came from (copied instead when the field changed or the saved action says Copy) and remembers the row as the last action used. The text is the one Rust holds for that row, never text from the page; an empty or missing result pastes nothing and the strip stays. Input: the session and the row id. Return: Err when there is nothing to paste.
#[tauri::command]
pub fn strip_paste(app: AppHandle, session: u64, id: String) -> Result<(), String> {
    let mut s = state();
    let found = s.session(session).and_then(|x| x.result_of(&id));
    let Some(text) = found.filter(|t| !t.trim().is_empty()) else { return Err("There is no result to paste.".into()) };
    let Some(session) = s.session.take() else { return Err("This strip is closed.".into()) };
    drop(s);
    dismiss(&app);
    settings::remember(LAST, id.clone().into());
    preview::deliver(&app, &id, &session.program, session.target, text);
    Ok(())
}

/// The page closes the strip (Escape on the strip, or a page error): nothing is pasted.
#[tauri::command]
pub fn strip_close(app: AppHandle) {
    dismiss(&app);
}
