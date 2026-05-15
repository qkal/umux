// SPDX-License-Identifier: GPL-3.0-or-later

use umux_core::{AppModel, SplitAxis, SurfaceKind};

#[test]
fn foundation_flow_creates_split_browser_and_unread_state() {
    let mut app = AppModel::new("C:/work/alpha");
    app.split_selected_pane(SplitAxis::Horizontal).unwrap();
    let browser = app
        .open_browser_surface("https://example.com".to_string())
        .unwrap();
    app.mark_surface_unread(browser, "needs review".to_string())
        .unwrap();

    let workspace = app.selected_workspace().unwrap();
    assert_eq!(workspace.panes.len(), 2);
    assert!(workspace.unread);
    assert_eq!(
        workspace.latest_notification.as_deref(),
        Some("needs review")
    );
    assert!(
        workspace
            .panes
            .iter()
            .flat_map(|pane| pane.surfaces.iter())
            .any(|surface| surface.kind == SurfaceKind::Browser)
    );
}
