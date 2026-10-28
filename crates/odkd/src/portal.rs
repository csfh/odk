//! The desktop's file chooser: `org.freedesktop.portal.FileChooser` on the session bus, asked as
//! `odkd` itself, a host application, so the portal answers with real paths (ADR 0068).

use std::path::{Path, PathBuf};

use ashpd::desktop::ResponseError;
use ashpd::desktop::file_chooser::{FileFilter, SelectedFiles};
use odk_theme::chooser::{ChooserAnswer, ChooserKind, ChooserRequest};

use crate::files::Chooser;

/// The portal on the session bus `DBUS_SESSION_BUS_ADDRESS` names.
pub struct Portal;

impl Chooser for Portal {
    fn choose(&self, request: &ChooserRequest, start: Option<&Path>) -> ChooserAnswer {
        futures_lite::future::block_on(ask(request, start))
    }
}

async fn ask(request: &ChooserRequest, start: Option<&Path>) -> ChooserAnswer {
    let filters = request.filters.iter().map(|f| {
        f.patterns
            .iter()
            .fold(FileFilter::new(&f.label), |filter, p| filter.glob(p))
    });
    let title = Some(request.title.as_str()).filter(|t| !t.is_empty());
    let accept = request.accept_label.as_deref();
    // The dialog is not attached to the application's window: there is no handle to give it.
    let sent = match request.kind {
        ChooserKind::Open | ChooserKind::Folder => {
            let mut r = SelectedFiles::open_file()
                .title(title)
                .accept_label(accept)
                .modal(false)
                .multiple(request.multiple)
                .directory(request.kind == ChooserKind::Folder)
                .filters(filters);
            if let Some(dir) = start {
                r = match r.current_folder(dir) {
                    Ok(r) => r,
                    Err(e) => return failed(e),
                };
            }
            r.send().await
        }
        ChooserKind::Save => {
            let mut r = SelectedFiles::save_file()
                .title(title)
                .accept_label(accept)
                .modal(false)
                .current_name(request.name.as_deref())
                .filters(filters);
            if let Some(dir) = start {
                r = match r.current_folder(dir) {
                    Ok(r) => r,
                    Err(e) => return failed(e),
                };
            }
            r.send().await
        }
    };
    match sent.and_then(|r| r.response()) {
        Ok(files) => {
            let paths: Vec<PathBuf> = files
                .uris()
                .iter()
                .filter(|u| u.scheme() == "file")
                .filter_map(|u| u.to_file_path().ok())
                .collect();
            if paths.is_empty() {
                ChooserAnswer::Cancelled
            } else {
                ChooserAnswer::Chosen(paths)
            }
        }
        Err(ashpd::Error::Response(ResponseError::Cancelled)) => ChooserAnswer::Cancelled,
        Err(ashpd::Error::PortalNotFound(_)) => {
            ChooserAnswer::Failed("the desktop has no file chooser portal".into())
        }
        Err(e) => failed(e),
    }
}

fn failed(e: impl std::fmt::Display) -> ChooserAnswer {
    ChooserAnswer::Failed(format!("the file chooser failed: {e}"))
}
