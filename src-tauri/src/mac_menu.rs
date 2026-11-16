//! macOS menü çubuğu (K107, faz M1). Mac'te Kopyala/Yapıştır gibi kısayollar
//! (Cmd+C/V) menüdeki öğelerden çalışır; menü olmazsa metin kutularında da
//! çalışmaz. Menü, launcher'ın diline göre Türkçe ya da İngilizce kurulur ve
//! dil değişince yenilenir. Windows/Linux'ta menü çubuğu yoktur; kod orada
//! da derlenir (ve test edilir) ama kullanılmaz.
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use tauri::menu::{AboutMetadata, Menu, PredefinedMenuItem, Submenu, WINDOW_SUBMENU_ID};
use tauri::{AppHandle, Runtime};

/// Menüdeki yazılar. Uygulama adı ayrı eklenir ("{app} Hakkında" gibi).
pub struct Labels {
    pub about: &'static str,
    pub services: &'static str,
    pub hide: &'static str,
    pub hide_others: &'static str,
    pub show_all: &'static str,
    pub quit: &'static str,
    pub edit: &'static str,
    pub undo: &'static str,
    pub redo: &'static str,
    pub cut: &'static str,
    pub copy: &'static str,
    pub paste: &'static str,
    pub select_all: &'static str,
    pub window: &'static str,
    pub minimize: &'static str,
    pub zoom: &'static str,
    pub fullscreen: &'static str,
    pub close_window: &'static str,
}

const TR: Labels = Labels {
    about: "{app} Hakkında",
    services: "Hizmetler",
    hide: "{app} Uygulamasını Gizle",
    hide_others: "Diğerlerini Gizle",
    show_all: "Tümünü Göster",
    quit: "{app} Uygulamasından Çık",
    edit: "Düzen",
    undo: "Geri Al",
    redo: "Yinele",
    cut: "Kes",
    copy: "Kopyala",
    paste: "Yapıştır",
    select_all: "Tümünü Seç",
    window: "Pencere",
    minimize: "Küçült",
    zoom: "Büyüt",
    fullscreen: "Tam Ekrana Geç",
    close_window: "Pencereyi Kapat",
};

const EN: Labels = Labels {
    about: "About {app}",
    services: "Services",
    hide: "Hide {app}",
    hide_others: "Hide Others",
    show_all: "Show All",
    quit: "Quit {app}",
    edit: "Edit",
    undo: "Undo",
    redo: "Redo",
    cut: "Cut",
    copy: "Copy",
    paste: "Paste",
    select_all: "Select All",
    window: "Window",
    minimize: "Minimize",
    zoom: "Zoom",
    fullscreen: "Enter Full Screen",
    close_window: "Close Window",
};

/// Arayüzün çözümlenmiş dili ("tr" / "en"); bilinmeyen her şey İngilizce.
pub fn labels(locale: &str) -> &'static Labels {
    if locale.eq_ignore_ascii_case("tr") {
        &TR
    } else {
        &EN
    }
}

/// Menüyü kurar: uygulama menüsü, Düzen ve Pencere. Kısayollar (Cmd+Q, Cmd+C,
/// Cmd+M…) macOS'un hazır öğelerinden gelir; yazılar bizim dilimizdedir.
pub fn build<R: Runtime>(app: &AppHandle<R>, locale: &str) -> tauri::Result<Menu<R>> {
    let l = labels(locale);
    let name = app.package_info().name.clone();
    let named = |text: &str| text.replace("{app}", &name);
    let about = AboutMetadata {
        name: Some(name.clone()),
        version: Some(app.package_info().version.to_string()),
        ..Default::default()
    };

    let app_menu = Submenu::with_items(
        app,
        &name,
        true,
        &[
            &PredefinedMenuItem::about(app, Some(&named(l.about)), Some(about))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::services(app, Some(l.services))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, Some(&named(l.hide)))?,
            &PredefinedMenuItem::hide_others(app, Some(l.hide_others))?,
            &PredefinedMenuItem::show_all(app, Some(l.show_all))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, Some(&named(l.quit)))?,
        ],
    )?;
    let edit_menu = Submenu::with_items(
        app,
        l.edit,
        true,
        &[
            &PredefinedMenuItem::undo(app, Some(l.undo))?,
            &PredefinedMenuItem::redo(app, Some(l.redo))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, Some(l.cut))?,
            &PredefinedMenuItem::copy(app, Some(l.copy))?,
            &PredefinedMenuItem::paste(app, Some(l.paste))?,
            &PredefinedMenuItem::select_all(app, Some(l.select_all))?,
        ],
    )?;
    // Bu kimlikle macOS, açık pencereleri bu menünün altında listeler.
    let window_menu = Submenu::with_id_and_items(
        app,
        WINDOW_SUBMENU_ID,
        l.window,
        true,
        &[
            &PredefinedMenuItem::minimize(app, Some(l.minimize))?,
            &PredefinedMenuItem::maximize(app, Some(l.zoom))?,
            &PredefinedMenuItem::fullscreen(app, Some(l.fullscreen))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::close_window(app, Some(l.close_window))?,
        ],
    )?;
    Menu::with_items(app, &[&app_menu, &edit_menu, &window_menu])
}

/// Menüyü verilen dilde kurar (yalnız macOS; başka yerde hiçbir şey yapmaz).
pub fn apply<R: Runtime>(app: &AppHandle<R>, locale: &str) -> tauri::Result<()> {
    #[cfg(target_os = "macos")]
    app.set_menu(build(app, locale)?)?;
    #[cfg(not(target_os = "macos"))]
    let _ = (app, locale);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_language_and_falls_back_to_english() {
        assert_eq!(labels("tr").copy, "Kopyala");
        assert_eq!(labels("TR").edit, "Düzen");
        assert_eq!(labels("en").copy, "Copy");
        assert_eq!(labels("de").paste, "Paste");
    }

    #[test]
    fn app_name_placeholder_only_where_expected() {
        for l in [&TR, &EN] {
            assert!(l.about.contains("{app}"));
            assert!(l.hide.contains("{app}"));
            assert!(l.quit.contains("{app}"));
            assert!(!l.copy.contains("{app}") && !l.window.contains("{app}"));
        }
    }
}
