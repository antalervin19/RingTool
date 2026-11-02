use crate::common::menu::Menu;
use crate::common::render::render_menu;
use crate::common::ring::{C, R1, R2, R3, R4, Ring};
use std::ffi::OsString;
use std::mem::size_of;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicIsize, Ordering};
use std::sync::{Mutex, OnceLock, RwLock};
use tiny_skia::Pixmap;
use windows::Win32::System::Com::{DVASPECT_CONTENT, FORMATETC, IDataObject, TYMED_HGLOBAL};
use windows::Win32::System::Ole::{CF_HDROP, ReleaseStgMedium};
use windows::Win32::System::Ole::{
    DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE, IDropTarget, IDropTarget_Impl, RegisterDragDrop,
};
use windows::Win32::System::Ole::{OleInitialize, OleUninitialize};
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_MENU};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_LMENU, VK_RMENU};
use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
use windows::Win32::UI::WindowsAndMessaging::{
    KBDLLHOOKSTRUCT, WH_KEYBOARD_LL, WM_KEYUP, WM_SYSKEYUP,
};
use windows::Win32::{
    Foundation::*, Graphics::Gdi::*, System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::*,
};
use windows::core::{Error, PCWSTR, Ref, Result, implement, w};

static MENU: OnceLock<RwLock<Menu>> = OnceLock::new();
static DRAGGED_FILE: OnceLock<Mutex<Vec<PathBuf>>> = OnceLock::new();
static DRAGGED_URL: OnceLock<Mutex<Option<String>>> = OnceLock::new();
static MENU_HWND: AtomicIsize = AtomicIsize::new(0);
static ACTIVE_CATEGORY: AtomicI32 = AtomicI32::new(-1);
static HOVERED_OPTION: AtomicI32 = AtomicI32::new(-1);
static DROP_TARGET_HOVER: AtomicBool = AtomicBool::new(false);

pub fn set_menu(menu: Menu) {
    MENU.set(RwLock::new(menu))
        .expect("Menu already initialized");
}

fn set_active_menu(menu: Menu) {
    let Some(menu_lock) = MENU.get() else {
        return;
    };

    *menu_lock.write().unwrap() = menu;

    ACTIVE_CATEGORY.store(-1, Ordering::Relaxed);
    HOVERED_OPTION.store(-1, Ordering::Relaxed);

    let hwnd_value = MENU_HWND.load(Ordering::Relaxed);
    if hwnd_value == 0 {
        return;
    }

    let hwnd = HWND(hwnd_value as *mut _);
    let menu = menu_lock.read().unwrap().clone();
    let pixmap = render_menu(&menu, None, None);
    let _ = present_pixmap(hwnd, &pixmap);
}

const WINDOW_CLASS: PCWSTR = w!("RingToolWindow");

static MOUSE_HOOK: OnceLock<isize> = OnceLock::new();
static KEYBOARD_HOOK: OnceLock<isize> = OnceLock::new();
static MOUSE_DOWN: AtomicBool = AtomicBool::new(false);
static ALT_DRAG_ACTIVE: AtomicBool = AtomicBool::new(false);
static DROP_SELECTION_VALID: AtomicBool = AtomicBool::new(false);

///// Dear Contributor.
/// When i wrote these lines only God & I knew how it worked.
/// Now not even God knows.
///
/// Please increment this counter to indicate how many hours you have spent untangling this mess.
///
/// Hours spent = 6.4

pub fn initialize_ole() -> Result<()> {
    unsafe {
        OleInitialize(None)?;
    }
    Ok(())
}

pub fn uninitialize_ole() {
    unsafe {
        OleUninitialize();
    }
}

unsafe extern "system" fn mouse_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if code >= 0 {
            match wparam.0 as u32 {
                WM_LBUTTONDOWN => {
                    MOUSE_DOWN.store(true, Ordering::Relaxed);
                }

                WM_LBUTTONUP => {
                    MOUSE_DOWN.store(false, Ordering::Relaxed);

                    if ALT_DRAG_ACTIVE.load(Ordering::Relaxed) {
                        if !DROP_SELECTION_VALID.load(Ordering::Relaxed) {
                            ALT_DRAG_ACTIVE.store(false, Ordering::Relaxed);
                            DROP_TARGET_HOVER.store(false, Ordering::Relaxed);
                            ACTIVE_CATEGORY.store(-1, Ordering::Relaxed);
                            HOVERED_OPTION.store(-1, Ordering::Relaxed);
                            DROP_SELECTION_VALID.store(false, Ordering::Relaxed);
                            hide_menu();
                        }
                    }
                }

                WM_MOUSEMOVE => {
                    let dragging = MOUSE_DOWN.load(Ordering::Relaxed);
                    let alt_down = GetAsyncKeyState(VK_MENU.0 as i32) < 0;

                    if dragging && alt_down {
                        if !ALT_DRAG_ACTIVE.swap(true, Ordering::Relaxed) {
                            show_menu_at_cursor();
                        }
                    } else if ALT_DRAG_ACTIVE.swap(false, Ordering::Relaxed) {
                        hide_menu();
                    }

                    if ALT_DRAG_ACTIVE.load(Ordering::Relaxed) {
                        forward_mouse_move_to_menu();
                    }
                }

                _ => {}
            }
        }

        CallNextHookEx(None, code, wparam, lparam)
    }
}

fn update_drop_selection(point: &POINTL) -> bool {
    let hwnd = HWND(MENU_HWND.load(Ordering::Relaxed) as *mut _);
    if hwnd.0.is_null() || unsafe { GetAsyncKeyState(VK_MENU.0 as i32) } >= 0 {
        return false;
    }

    let mut rect = RECT::default();

    if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
        return false;
    }

    let x = point.x - rect.left;
    let y = point.y - rect.top;
    let dx = x as f32 - C;
    let dy = y as f32 - C;

    if x < 0 || y < 0 || x >= 460 || y >= 460 {
        return false;
    }

    let position = ((y as u32 & 0xFFFF) << 16) | (x as u32 & 0xFFFF);

    unsafe {
        SendMessageW(
            hwnd,
            WM_MOUSEMOVE,
            Some(WPARAM(0)),
            Some(LPARAM(position as isize)),
        );
    }

    let Some(menu_lock) = MENU.get() else {
        return false;
    };

    let menu = menu_lock.read().unwrap().clone();

    let option = HOVERED_OPTION.load(Ordering::Relaxed);

    match &menu {
        Menu::SingleRing { options } => option >= 0 && (option as usize) < options.len(),

        Menu::MultiRing { categories } => {
            let category = ACTIVE_CATEGORY.load(Ordering::Relaxed);

            category >= 0
                && (category as usize) < categories.len()
                && option >= 0
                && (option as usize) < categories[category as usize].options.len()
        }
    }
}

pub fn install_mouse_hook() -> Result<()> {
    let hook = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), None, 0)? };

    MOUSE_HOOK
        .set(hook.0 as isize)
        .expect("Mouse hook already installed");

    Ok(())
}

pub fn create_window() -> Result<HWND> {
    unsafe {
        let instance = GetModuleHandleW(None)?.into();

        let class = WNDCLASSW {
            hInstance: instance,
            lpszClassName: WINDOW_CLASS,
            lpfnWndProc: Some(window_proc),
            ..Default::default()
        };

        RegisterClassW(&class);

        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            WINDOW_CLASS,
            w!("RingTool"),
            WS_POPUP,
            0,
            0,
            460,
            460,
            None,
            None,
            Some(instance),
            None,
        )?;

        let drop_target: IDropTarget = RingDropTarget.into();
        RegisterDragDrop(hwnd, &drop_target)?;

        MENU_HWND.store(hwnd.0 as isize, Ordering::Relaxed);
        Ok(hwnd)
    }
}

pub fn show_menu_at_cursor() {
    unsafe {
        let hwnd_value = MENU_HWND.load(Ordering::Relaxed);
        if hwnd_value == 0 {
            return;
        }

        let hwnd = HWND(hwnd_value as *mut _);
        let mut cursor = POINT::default();

        if GetCursorPos(&mut cursor).is_err() {
            return;
        }

        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            cursor.x - C as i32,
            cursor.y - C as i32,
            460,
            460,
            SWP_NOACTIVATE,
        );

        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    }
}

pub fn hide_menu() {
    let hwnd_value = MENU_HWND.load(Ordering::Relaxed);

    if hwnd_value != 0 {
        unsafe {
            let _ = ShowWindow(HWND(hwnd_value as *mut _), SW_HIDE);
        }
    }
}

unsafe extern "system" fn keyboard_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if code >= 0 && matches!(wparam.0 as u32, WM_KEYUP | WM_SYSKEYUP) {
            let key = *(lparam.0 as *const KBDLLHOOKSTRUCT);

            if [VK_MENU.0 as u32, VK_LMENU.0 as u32, VK_RMENU.0 as u32].contains(&key.vkCode) {
                if ALT_DRAG_ACTIVE.swap(false, Ordering::Relaxed) {
                    DROP_TARGET_HOVER.store(false, Ordering::Relaxed);
                    ACTIVE_CATEGORY.store(-1, Ordering::Relaxed);
                    HOVERED_OPTION.store(-1, Ordering::Relaxed);
                    hide_menu();
                }
            }
        }

        CallNextHookEx(None, code, wparam, lparam)
    }
}

pub fn install_keyboard_hook() -> Result<()> {
    unsafe {
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), None, 0)?;

        KEYBOARD_HOOK.set(hook.0 as isize).ok();

        Ok(())
    }
}

fn forward_mouse_move_to_menu() {
    unsafe {
        let hwnd_value = MENU_HWND.load(Ordering::Relaxed);
        if hwnd_value == 0 {
            return;
        }

        let hwnd = HWND(hwnd_value as *mut _);
        let mut cursor = POINT::default();

        if GetCursorPos(&mut cursor).is_err() {
            return;
        }

        if !ScreenToClient(hwnd, &mut cursor).as_bool() {
            return;
        }

        let x = cursor.x as u16 as u32;
        let y = cursor.y as u16 as u32;
        let position = LPARAM(((y << 16) | x) as isize);

        let _ = PostMessageW(Some(hwnd), WM_MOUSEMOVE, WPARAM(0), position);
    }
}

pub fn present_pixmap(hwnd: HWND, pixmap: &Pixmap) -> Result<()> {
    unsafe {
        let width = pixmap.width() as i32;
        let height = pixmap.height() as i32;

        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = width;
        bmi.bmiHeader.biHeight = -height;
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB.0;

        let hdc = CreateCompatibleDC(None);
        if hdc.is_invalid() {
            return Err(Error::from_thread());
        }

        let mut bits = null_mut();

        let bitmap = match CreateDIBSection(Some(hdc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0) {
            Ok(bitmap) => bitmap,
            Err(error) => {
                let _ = DeleteDC(hdc);
                return Err(error);
            }
        };

        if bits.is_null() {
            let _ = DeleteObject(bitmap.into());
            let _ = DeleteDC(hdc);
            return Err(Error::from_thread());
        }

        let dst =
            std::slice::from_raw_parts_mut(bits as *mut u8, width as usize * height as usize * 4);

        let menu_radius = match MENU.get().map(|menu| menu.read().unwrap().clone()) {
            Some(Menu::SingleRing { .. }) => R2,
            Some(Menu::MultiRing { .. }) => R4,
            None => R4,
        };
        let menu_radius_sq = menu_radius * menu_radius;

        for (i, (src, dst)) in pixmap
            .data()
            .chunks_exact(4)
            .zip(dst.chunks_exact_mut(4))
            .enumerate()
        {
            let x = (i % width as usize) as f32;
            let y = (i / width as usize) as f32;
            let dx = x - C;
            let dy = y - C;

            if dx * dx + dy * dy <= menu_radius_sq {
                dst[0] = src[2];
                dst[1] = src[1];
                dst[2] = src[0];
                dst[3] = src[3].max(1);
            } else {
                dst[0] = src[2];
                dst[1] = src[1];
                dst[2] = src[0];
                dst[3] = src[3];
            }
        }

        let previous = SelectObject(hdc, bitmap.into());

        let mut rect = RECT::default();
        let _ = GetWindowRect(hwnd, &mut rect);

        let position = POINT {
            x: rect.left,
            y: rect.top,
        };
        let source = POINT { x: 0, y: 0 };
        let size = SIZE {
            cx: width,
            cy: height,
        };

        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };

        let result = UpdateLayeredWindow(
            hwnd,
            None,
            Some(&position),
            Some(&size),
            Some(hdc),
            Some(&source),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        );

        let _ = SelectObject(hdc, previous);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(hdc);

        result
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),

            WM_MOUSEMOVE => {
                let Some(menu_lock) = MENU.get() else {
                    return LRESULT(0);
                };

                let menu = menu_lock.read().unwrap().clone();

                let x = (lparam.0 as u16 as i16) as f32;
                let y = ((lparam.0 >> 16) as u16 as i16) as f32;

                let dx = x - C;
                let dy = y - C;

                let (category, option) = match &menu {
                    Menu::SingleRing { options } => {
                        let option = if options.is_empty() {
                            None
                        } else {
                            let ring = Ring {
                                r1: R1,
                                r2: R2,
                                first: 0.0,
                                step: 360.0 / options.len() as f32,
                                count: options.len(),
                            };

                            ring.hit(dx, dy)
                        };

                        (None, option)
                    }

                    Menu::MultiRing { categories } => {
                        if categories.is_empty() {
                            return LRESULT(0);
                        }

                        let category_step = 360.0 / categories.len() as f32;

                        let category_ring = Ring {
                            r1: R1,
                            r2: R2,
                            first: 0.0,
                            step: category_step,
                            count: categories.len(),
                        };

                        let gap_ring = Ring {
                            r1: R2,
                            r2: R3,
                            first: 0.0,
                            step: category_step,
                            count: categories.len(),
                        };

                        let current = ACTIVE_CATEGORY.load(Ordering::Relaxed);

                        let category = category_ring
                            .hit(dx, dy)
                            .or_else(|| gap_ring.hit(dx, dy))
                            .or_else(|| {
                                if current < 0 || current as usize >= categories.len() {
                                    return None;
                                }

                                let distance = dx * dx + dy * dy;

                                if (R3 * R3..=R4 * R4).contains(&distance) {
                                    Some(current as usize)
                                } else {
                                    None
                                }
                            });

                        let option = category.and_then(|index| {
                            let options = &categories[index].options;

                            if options.is_empty() {
                                return None;
                            }

                            let option_step = category_step / options.len() as f32;

                            let ring = Ring {
                                r1: R3,
                                r2: R4,
                                first: index as f32 * category_step - category_step / 2.0
                                    + option_step / 2.0,
                                step: option_step,
                                count: options.len(),
                            };

                            ring.hit(dx, dy)
                        });

                        (category, option)
                    }
                };

                let new_category = category.map_or(-1, |i| i as i32);
                let new_option = option.map_or(-1, |i| i as i32);

                let old_category = ACTIVE_CATEGORY.swap(new_category, Ordering::Relaxed);
                let old_option = HOVERED_OPTION.swap(new_option, Ordering::Relaxed);

                if old_category != new_category || old_option != new_option {
                    let pm = render_menu(&menu, category, option);
                    let _ = present_pixmap(hwnd, &pm);
                }

                LRESULT(0)
            }

            WM_SETCURSOR => {
                let cursor = LoadCursorW(None, IDC_ARROW).unwrap();
                let _ = SetCursor(Some(cursor));
                LRESULT(1)
            }

            WM_CLOSE => {
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }

            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }

            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

pub fn run_message_loop() {
    unsafe {
        let mut msg = MSG::default();

        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

#[implement(IDropTarget)]
struct RingDropTarget;

#[allow(non_snake_case)]
impl IDropTarget_Impl for RingDropTarget_Impl {
fn DragEnter(
    &self,
        data: Ref<'_, IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> Result<()> {
        DROP_TARGET_HOVER.store(true, Ordering::Relaxed);

        let paths = data.as_ref().map(get_dragged_files).unwrap_or_default();
        let url = if paths.is_empty() {
            data.as_ref().and_then(get_dragged_url)
        } else {
            None
        };

        *DRAGGED_FILE
            .get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .unwrap() = paths.clone();
        *DRAGGED_URL
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap() = url.clone();

        let menu = match &url {
            Some(_) => Some(crate::common::menu::menu_for_url()),
            None => crate::common::menu::menu_for_files(&paths),
        }
        .unwrap_or(Menu::SingleRing {
            options: Vec::new(),
        });

        set_active_menu(menu);

        let valid = update_drop_selection(point);
        DROP_SELECTION_VALID.store(valid, Ordering::Relaxed);

        unsafe {
            *effect = if valid {
                DROPEFFECT_COPY
            } else {
                DROPEFFECT_NONE
            };
        }

        Ok(())
    }

    fn DragOver(
        &self,
        _keys: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> Result<()> {
        DROP_TARGET_HOVER.store(true, Ordering::Relaxed);

        let valid = update_drop_selection(point);
        DROP_SELECTION_VALID.store(valid, Ordering::Relaxed);

        unsafe {
            *effect = if valid {
                DROPEFFECT_COPY
            } else {
                DROPEFFECT_NONE
            };
        }

        Ok(())
    }

    fn DragLeave(&self) -> Result<()> {
        DROP_TARGET_HOVER.store(false, Ordering::Relaxed);
        DROP_SELECTION_VALID.store(false, Ordering::Relaxed);
        if let Some(file) = DRAGGED_FILE.get() {
            file.lock().unwrap().clear();
        }
        if let Some(url) = DRAGGED_URL.get() {
            *url.lock().unwrap() = None;
        }
        Ok(())
    }

    fn Drop(
        &self,
        _data: Ref<'_, IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> Result<()> {
        let valid = update_drop_selection(point);

        unsafe {
            *effect = if valid {
                DROPEFFECT_COPY
            } else {
                DROPEFFECT_NONE
            };
        }

        DROP_TARGET_HOVER.store(false, Ordering::Relaxed);
        DROP_SELECTION_VALID.store(false, Ordering::Relaxed);

        let selected = if valid { selected_option() } else { None };
        let files = DRAGGED_FILE
            .get()
            .map(|f| std::mem::take(&mut *f.lock().unwrap()))
            .unwrap_or_default();
        let url = DRAGGED_URL
            .get()
            .and_then(|u| u.lock().unwrap().take());

        hide_menu();

        if let Some(option) = selected {
            std::thread::spawn(move || {
                let name = match &url {
                    Some(url) => url.clone(),
                    None => files
                        .iter()
                        .filter_map(|f| f.file_name()?.to_str().map(str::to_owned))
                        .collect::<Vec<_>>()
                        .join(" + "),
                };
                let target = option.output_format.to_uppercase();

                notify("Conversion started", &format!("{name} → {target}"));

                let result = match &url {
                    Some(url) => crate::common::convert::download_url(
                        url,
                        &option.id,
                        &option.output_format,
                        if crate::common::settings::get().use_explorer_folder { explorer_folder() } else { None },
                    ),
                    None => crate::common::convert::convert(&files, &option.id, &option.output_format),
                };

                match result {
                    Ok(path) => {
                        println!("Converted: {}", path.display());
                        notify("Conversion finished", &path.display().to_string());
                    }
                    Err(error) => {
                        eprintln!("Conversion failed: {error}");
                        let last = error.lines().last().unwrap_or("Unknown error");
                        notify("Conversion failed", &format!("{name}: {last}"));
                    }
                }
            });
        }

        Ok(())
    }
}

unsafe extern "system" fn collect_explorer_windows(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
    unsafe {
        let found = &mut *(lparam.0 as *mut Option<HWND>);
        let mut class = [0u16; 64];
        let len = GetClassNameW(hwnd, &mut class) as usize;

        if IsWindowVisible(hwnd).as_bool()
            && !IsIconic(hwnd).as_bool()
            && String::from_utf16_lossy(&class[..len]) == "CabinetWClass"
        {
            *found = Some(hwnd);
            return windows::core::BOOL(0);
        }

        windows::core::BOOL(1)
    }
}

pub fn explorer_folder() -> Option<PathBuf> {
    use std::os::windows::process::CommandExt;

    let mut found: Option<HWND> = None;
    unsafe {
        let _ = EnumWindows(
            Some(collect_explorer_windows),
            LPARAM(&mut found as *mut _ as isize),
        );
    }
    let hwnd = found?;

    let script = format!(
        "$h = {}; foreach ($w in (New-Object -ComObject Shell.Application).Windows()) {{ if ($w.HWND -eq $h) {{ $w.Document.Folder.Self.Path; break }} }}",
        hwnd.0 as isize
    );

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(0x0800_0000)
        .output()
        .ok()?;

    let path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    path.is_dir().then_some(path)
}

pub const APP_ID: &str = "RingTool";

pub fn register_app_id() {
    use winreg::{RegKey, enums::HKEY_CURRENT_USER};

    let result = (|| -> std::io::Result<()> {
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
            .create_subkey(format!(r"Software\Classes\AppUserModelId\{APP_ID}"))?;
        key.set_value("DisplayName", &"RingTool")?;

        let icon_dir = std::env::var_os("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("RingTool");
        std::fs::create_dir_all(&icon_dir)?;
        let icon_path = icon_dir.join("icon.ico");
        std::fs::write(
            &icon_path,
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/icon.ico")),
        )?;
        key.set_value("IconUri", &icon_path.to_string_lossy().as_ref())?;
        Ok(())
    })();

    if let Err(error) = result {
        eprintln!("Failed to register notification app id: {error}");
    }
}

fn notify(title: &str, body: &str) {
    let _ = winrt_notification::Toast::new(APP_ID)
        .title(title)
        .text1(body)
        .show();
}

pub fn notify_updated(version: &str) {
    let _ = winrt_notification::Toast::new(APP_ID)
        .title("RingTool")
        .text1("RingTool was updated in the background")
        .text2(&format!("Now running Latest: {version}"))
        .show();
}

fn selected_option() -> Option<crate::common::menu::MenuOption> {
    let menu = MENU.get()?.read().unwrap().clone();
    let option = usize::try_from(HOVERED_OPTION.load(Ordering::Relaxed)).ok()?;

    match menu {
        Menu::SingleRing { options } => options.get(option).cloned(),
        Menu::MultiRing { categories } => {
            let category = usize::try_from(ACTIVE_CATEGORY.load(Ordering::Relaxed)).ok()?;
            categories.get(category)?.options.get(option).cloned()
        }
    }
}

fn read_hglobal_text(
    data: &IDataObject,
    format: u16,
    wide: bool,
) -> Option<String> {
    use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

    let fmt = FORMATETC {
        cfFormat: format,
        ptd: null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    };

    let mut medium = unsafe { data.GetData(&fmt).ok()? };

    let text = unsafe {
        let handle = medium.u.hGlobal;
        let size = GlobalSize(handle);
        let ptr = GlobalLock(handle);

        let text = if ptr.is_null() {
            None
        } else if wide {
            let units = std::slice::from_raw_parts(ptr as *const u16, size / 2);
            let end = units.iter().position(|&c| c == 0).unwrap_or(units.len());
            Some(String::from_utf16_lossy(&units[..end]))
        } else {
            let bytes = std::slice::from_raw_parts(ptr as *const u8, size);
            let end = bytes.iter().position(|&c| c == 0).unwrap_or(bytes.len());
            Some(String::from_utf8_lossy(&bytes[..end]).into_owned())
        };

        if !ptr.is_null() {
            let _ = GlobalUnlock(handle);
        }
        text
    };

    unsafe {
        ReleaseStgMedium(&mut medium);
    }

    text
}

fn get_dragged_url(data: &IDataObject) -> Option<String> {
    let url_w = unsafe { windows::Win32::System::DataExchange::RegisterClipboardFormatW(w!("UniformResourceLocatorW")) } as u16;
    let url_a = unsafe { windows::Win32::System::DataExchange::RegisterClipboardFormatW(w!("UniformResourceLocator")) } as u16;

    let candidates = [
        read_hglobal_text(data, url_w, true),
        read_hglobal_text(data, url_a, false),
        read_hglobal_text(data, 13, true),
    ];

    candidates
        .into_iter()
        .flatten()
        .map(|s| s.trim().to_string())
        .find(|s| {
            (s.starts_with("http://") || s.starts_with("https://")) && !s.contains(char::is_whitespace)
        })
}

fn get_dragged_files(data: &IDataObject) -> Vec<PathBuf> {
    let format = FORMATETC {
        cfFormat: CF_HDROP.0,
        ptd: null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    };

    let Ok(mut medium) = (unsafe { data.GetData(&format) }) else {
        return Vec::new();
    };

    let mut paths = Vec::new();

    unsafe {
        let hdrop = HDROP(medium.u.hGlobal.0);
        let count = DragQueryFileW(hdrop, u32::MAX, None);

        for index in 0..count {
            let length = DragQueryFileW(hdrop, index, None);
            if length == 0 {
                continue;
            }

            let mut buffer = vec![0u16; length as usize + 1];
            let written = DragQueryFileW(hdrop, index, Some(&mut buffer));

            if written > 0 {
                paths.push(PathBuf::from(OsString::from_wide(
                    &buffer[..written as usize],
                )));
            }
        }

        ReleaseStgMedium(&mut medium);
    }

    paths
}


pub fn request_quit() {
    let hwnd_value = MENU_HWND.load(Ordering::Relaxed);

    if hwnd_value != 0 {
        unsafe {
            let _ = PostMessageW(
                Some(HWND(hwnd_value as *mut _)),
                WM_CLOSE,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }
}
