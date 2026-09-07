mod app;

use std::path::PathBuf;

fn main() {
    let open_path = std::env::args().nth(1).map(PathBuf::from);

    let app = gpui_kit::application().with_assets(gpui_kit::assets::Assets);
    app.run(move |cx| {
        gpui_kit::init(cx);
        app::init(cx);
        cx.activate(true);

        let path = open_path.clone();
        cx.spawn(async move |cx| {
            app::open_window(path, cx).expect("failed to open window");
        })
        .detach();
    });
}
