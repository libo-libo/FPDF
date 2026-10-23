use std::path::{Path, PathBuf};

pub fn pick_file(title: &str, filter_name: &str, extension: &str) -> Option<PathBuf> {
    pick_files_inner(title, filter_name, extension, false).and_then(|mut paths| paths.pop())
}

pub fn pick_files(title: &str, filter_name: &str, extension: &str) -> Option<Vec<PathBuf>> {
    pick_files_inner(title, filter_name, extension, true)
}

pub fn save_file(
    title: &str,
    filter_name: &str,
    extension: &str,
    directory: Option<&Path>,
    file_name: Option<&str>,
) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let mut dialog = rfd::FileDialog::new()
            .set_title(title)
            .add_filter(filter_name, &[extension]);
        if let Some(directory) = directory {
            dialog = dialog.set_directory(directory);
        }
        if let Some(file_name) = file_name {
            dialog = dialog.set_file_name(file_name);
        }
        return dialog.save_file();
    }
    #[cfg(target_os = "linux")]
    {
        return gtk::run(
            title,
            filter_name,
            extension,
            GtkMode::Save,
            directory,
            file_name,
        )
        .and_then(|mut paths| paths.pop());
    }
    #[allow(unreachable_code)]
    None
}

fn pick_files_inner(
    title: &str,
    filter_name: &str,
    extension: &str,
    multiple: bool,
) -> Option<Vec<PathBuf>> {
    #[cfg(windows)]
    {
        let dialog = rfd::FileDialog::new()
            .set_title(title)
            .add_filter(filter_name, &[extension]);
        return if multiple {
            dialog.pick_files()
        } else {
            dialog.pick_file().map(|path| vec![path])
        };
    }
    #[cfg(target_os = "linux")]
    {
        let mode = if multiple {
            GtkMode::OpenMany
        } else {
            GtkMode::OpenOne
        };
        return gtk::run(title, filter_name, extension, mode, None, None);
    }
    #[allow(unreachable_code)]
    None
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy)]
enum GtkMode {
    OpenOne,
    OpenMany,
    Save,
}

#[cfg(target_os = "linux")]
mod gtk {
    use super::GtkMode;
    use libloading::Library;
    use std::ffi::{CStr, CString, c_char, c_int, c_void};
    use std::path::{Path, PathBuf};
    use std::ptr;
    use std::sync::OnceLock;

    const GTK_FILE_CHOOSER_ACTION_OPEN: c_int = 0;
    const GTK_FILE_CHOOSER_ACTION_SAVE: c_int = 1;
    const GTK_RESPONSE_ACCEPT: c_int = -3;
    const GTK_RESPONSE_CANCEL: c_int = -6;

    #[repr(C)]
    struct GSList {
        data: *mut c_void,
        next: *mut GSList,
    }

    struct Api {
        _gtk: Library,
        _glib: Library,
        init_check: unsafe extern "C" fn(*mut c_int, *mut *mut *mut c_char) -> c_int,
        dialog_new: unsafe extern "C" fn(
            *const c_char,
            *mut c_void,
            c_int,
            *const c_char,
            ...
        ) -> *mut c_void,
        set_select_multiple: unsafe extern "C" fn(*mut c_void, c_int),
        set_current_folder: unsafe extern "C" fn(*mut c_void, *const c_char) -> c_int,
        set_current_name: unsafe extern "C" fn(*mut c_void, *const c_char),
        add_filter: unsafe extern "C" fn(*mut c_void, *mut c_void),
        filter_new: unsafe extern "C" fn() -> *mut c_void,
        filter_set_name: unsafe extern "C" fn(*mut c_void, *const c_char),
        filter_add_pattern: unsafe extern "C" fn(*mut c_void, *const c_char),
        get_filename: unsafe extern "C" fn(*mut c_void) -> *mut c_char,
        get_filenames: unsafe extern "C" fn(*mut c_void) -> *mut GSList,
        dialog_run: unsafe extern "C" fn(*mut c_void) -> c_int,
        widget_destroy: unsafe extern "C" fn(*mut c_void),
        slist_free: unsafe extern "C" fn(*mut GSList),
        free: unsafe extern "C" fn(*mut c_void),
    }

    impl Api {
        fn load() -> Option<Self> {
            // The target distribution ships GTK 3 without development headers. Loading
            // the stable C ABI keeps the release build independent of GTK dev packages.
            unsafe {
                let gtk = Library::new("libgtk-3.so.0").ok()?;
                let glib = Library::new("libglib-2.0.so.0").ok()?;
                macro_rules! symbol {
                    ($library:expr, $name:literal, $type:ty) => {
                        *$library
                            .get::<$type>(concat!($name, "\0").as_bytes())
                            .ok()?
                    };
                }
                let api = Self {
                    init_check: symbol!(
                        gtk,
                        "gtk_init_check",
                        unsafe extern "C" fn(*mut c_int, *mut *mut *mut c_char) -> c_int
                    ),
                    dialog_new: symbol!(
                        gtk,
                        "gtk_file_chooser_dialog_new",
                        unsafe extern "C" fn(
                            *const c_char,
                            *mut c_void,
                            c_int,
                            *const c_char,
                            ...
                        ) -> *mut c_void
                    ),
                    set_select_multiple: symbol!(
                        gtk,
                        "gtk_file_chooser_set_select_multiple",
                        unsafe extern "C" fn(*mut c_void, c_int)
                    ),
                    set_current_folder: symbol!(
                        gtk,
                        "gtk_file_chooser_set_current_folder",
                        unsafe extern "C" fn(*mut c_void, *const c_char) -> c_int
                    ),
                    set_current_name: symbol!(
                        gtk,
                        "gtk_file_chooser_set_current_name",
                        unsafe extern "C" fn(*mut c_void, *const c_char)
                    ),
                    add_filter: symbol!(
                        gtk,
                        "gtk_file_chooser_add_filter",
                        unsafe extern "C" fn(*mut c_void, *mut c_void)
                    ),
                    filter_new: symbol!(
                        gtk,
                        "gtk_file_filter_new",
                        unsafe extern "C" fn() -> *mut c_void
                    ),
                    filter_set_name: symbol!(
                        gtk,
                        "gtk_file_filter_set_name",
                        unsafe extern "C" fn(*mut c_void, *const c_char)
                    ),
                    filter_add_pattern: symbol!(
                        gtk,
                        "gtk_file_filter_add_pattern",
                        unsafe extern "C" fn(*mut c_void, *const c_char)
                    ),
                    get_filename: symbol!(
                        gtk,
                        "gtk_file_chooser_get_filename",
                        unsafe extern "C" fn(*mut c_void) -> *mut c_char
                    ),
                    get_filenames: symbol!(
                        gtk,
                        "gtk_file_chooser_get_filenames",
                        unsafe extern "C" fn(*mut c_void) -> *mut GSList
                    ),
                    dialog_run: symbol!(
                        gtk,
                        "gtk_dialog_run",
                        unsafe extern "C" fn(*mut c_void) -> c_int
                    ),
                    widget_destroy: symbol!(
                        gtk,
                        "gtk_widget_destroy",
                        unsafe extern "C" fn(*mut c_void)
                    ),
                    slist_free: symbol!(glib, "g_slist_free", unsafe extern "C" fn(*mut GSList)),
                    free: symbol!(glib, "g_free", unsafe extern "C" fn(*mut c_void)),
                    _gtk: gtk,
                    _glib: glib,
                };
                let mut argc: c_int = 0;
                let mut argv: *mut *mut c_char = ptr::null_mut();
                if (api.init_check)(&mut argc, &mut argv) == 0 {
                    return None;
                }
                Some(api)
            }
        }
    }

    fn api() -> Option<&'static Api> {
        static API: OnceLock<Option<Api>> = OnceLock::new();
        API.get_or_init(Api::load).as_ref()
    }

    fn path_from_gtk(api: &Api, value: *mut c_char) -> Option<PathBuf> {
        if value.is_null() {
            return None;
        }
        let path = unsafe { CStr::from_ptr(value) }
            .to_string_lossy()
            .into_owned();
        unsafe { (api.free)(value.cast()) };
        Some(PathBuf::from(path))
    }

    pub fn run(
        title: &str,
        filter_name: &str,
        extension: &str,
        mode: GtkMode,
        directory: Option<&Path>,
        file_name: Option<&str>,
    ) -> Option<Vec<PathBuf>> {
        let api = api()?;
        let title = CString::new(title).ok()?;
        let filter_name = CString::new(filter_name).ok()?;
        let pattern = CString::new(format!("*.{extension}")).ok()?;
        let accept = CString::new("打开").ok()?;
        let save = CString::new("保存").ok()?;
        let cancel = CString::new("取消").ok()?;
        let (action, accept_label) = match mode {
            GtkMode::Save => (GTK_FILE_CHOOSER_ACTION_SAVE, save.as_ptr()),
            _ => (GTK_FILE_CHOOSER_ACTION_OPEN, accept.as_ptr()),
        };
        let dialog = unsafe {
            (api.dialog_new)(
                title.as_ptr(),
                ptr::null_mut(),
                action,
                accept_label,
                GTK_RESPONSE_ACCEPT,
                cancel.as_ptr(),
                GTK_RESPONSE_CANCEL,
                ptr::null::<c_char>(),
            )
        };
        if dialog.is_null() {
            return None;
        }
        let filter = unsafe { (api.filter_new)() };
        if !filter.is_null() {
            unsafe {
                (api.filter_set_name)(filter, filter_name.as_ptr());
                (api.filter_add_pattern)(filter, pattern.as_ptr());
                (api.add_filter)(dialog, filter);
            }
        }
        if matches!(mode, GtkMode::OpenMany) {
            unsafe { (api.set_select_multiple)(dialog, 1) };
        }
        if let Some(directory) =
            directory.and_then(|path| CString::new(path.to_string_lossy().as_bytes()).ok())
        {
            unsafe { (api.set_current_folder)(dialog, directory.as_ptr()) };
        }
        if let Some(file_name) = file_name.and_then(|name| CString::new(name).ok()) {
            unsafe { (api.set_current_name)(dialog, file_name.as_ptr()) };
        }
        let response = unsafe { (api.dialog_run)(dialog) };
        let paths = if response == GTK_RESPONSE_ACCEPT {
            match mode {
                GtkMode::OpenMany => unsafe {
                    let mut result = Vec::new();
                    let mut node = (api.get_filenames)(dialog);
                    let head = node;
                    while !node.is_null() {
                        if let Some(path) = path_from_gtk(api, (*node).data.cast()) {
                            result.push(path);
                        }
                        node = (*node).next;
                    }
                    (api.slist_free)(head);
                    (!result.is_empty()).then_some(result)
                },
                _ => {
                    path_from_gtk(api, unsafe { (api.get_filename)(dialog) }).map(|path| vec![path])
                }
            }
        } else {
            None
        };
        unsafe { (api.widget_destroy)(dialog) };
        paths
    }
}
