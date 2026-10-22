use eframe::egui;
use std::collections::HashMap;
use std::path::Path;
use std::process::{Child, Command, Stdio};

pub fn spawn_window(path: Option<&Path>) -> Result<Child, std::io::Error> {
    window_command(path)?.spawn()
}

pub fn window_command(path: Option<&Path>) -> Result<Command, std::io::Error> {
    let mut command = Command::new(std::env::current_exe()?);
    if let Some(path) = path {
        command.arg(path);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    Ok(command)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UninstallChoice {
    Keep,
    Cancel,
    Confirm,
}

pub fn confirm_uninstall(
    ctx: &egui::Context,
    controls: &mut HashMap<&'static str, egui::Rect>,
) -> UninstallChoice {
    let mut choice = UninstallChoice::Keep;
    egui::Modal::new(egui::Id::new("confirm-uninstall")).show(ctx,|ui| {
        ui.set_max_width(420.0);
        ui.heading("确认卸载 fpdf？");
        ui.label("将关闭本安装目录下的所有 fpdf 窗口，并清除程序、已保存的 AI 配置与密钥、主题、缓存和日志。");
        ui.label("用户 PDF、外部配置文件和其他便携实例将保留。");
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            let cancel=ui.button("取消");
            controls.insert("uninstall-cancel",cancel.rect);
            if cancel.clicked() {choice=UninstallChoice::Cancel;}
            let confirm=ui.add(egui::Button::new(egui::RichText::new("确认卸载并清除配置").color(egui::Color32::WHITE)).fill(egui::Color32::from_rgb(218,51,54)));
            controls.insert("uninstall-confirm",confirm.rect);
            if confirm.clicked() {choice=UninstallChoice::Confirm;}
        });
    });
    if choice == UninstallChoice::Keep
        && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
    {
        choice = UninstallChoice::Cancel;
    }
    choice
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uninstall_requires_an_explicit_choice_and_cancel_never_confirms() {
        for (control, expected) in [
            ("uninstall-cancel", UninstallChoice::Cancel),
            ("uninstall-confirm", UninstallChoice::Confirm),
        ] {
            let ctx = egui::Context::default();
            let mut controls = HashMap::new();
            let input = |events| egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 800.0),
                )),
                events,
                ..Default::default()
            };
            for _ in 0..2 {
                let _ = ctx.run(input(vec![]), |ctx| {
                    assert_eq!(confirm_uninstall(ctx, &mut controls), UninstallChoice::Keep);
                });
            }
            let pos = controls[control].center();
            for pressed in [true, false] {
                let mut result = UninstallChoice::Keep;
                let _ = ctx.run(
                    input(vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ]),
                    |ctx| {
                        let next = confirm_uninstall(ctx, &mut controls);
                        if next != UninstallChoice::Keep {
                            result = next;
                        }
                    },
                );
                if !pressed {
                    assert_eq!(result, expected);
                }
            }
        }
    }
}
