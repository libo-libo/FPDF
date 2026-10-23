use fpdf::reading::PaperTheme;
use std::path::PathBuf;

pub struct ThemeSettings {
    pub theme: PaperTheme,
    pub rgb: [u8; 3],
}
impl Default for ThemeSettings {
    fn default() -> Self {
        Self {
            theme: PaperTheme::White,
            rgb: [225, 237, 248],
        }
    }
}
impl ThemeSettings {
    fn path() -> Option<PathBuf> {
        crate::platform::config_path("reading-theme.txt")
    }
    pub fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| Self::parse(&s))
            .unwrap_or_default()
    }
    fn parse(text: &str) -> Option<Self> {
        let mut fields = text.split_whitespace();
        let kind = fields.next()?;
        let rgb = [
            fields.next()?.parse().ok()?,
            fields.next()?.parse().ok()?,
            fields.next()?.parse().ok()?,
        ];
        let theme = match kind {
            "white" => PaperTheme::White,
            "green" => PaperTheme::Green,
            "warm" => PaperTheme::Warm,
            "night" => PaperTheme::Night,
            "custom" => PaperTheme::Custom(rgb),
            _ => return None,
        };
        Some(Self { theme, rgb })
    }
    pub fn save(&self) -> Result<(), String> {
        let path = Self::path().ok_or("无法找到用户配置目录。")?;
        self.write_to(&path)
    }
    fn write_to(&self, path: &std::path::Path) -> Result<(), String> {
        let parent = path.parent().ok_or("配置目录无效。")?;
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        let kind = match self.theme {
            PaperTheme::White => "white",
            PaperTheme::Green => "green",
            PaperTheme::Warm => "warm",
            PaperTheme::Night => "night",
            PaperTheme::Custom(_) => "custom",
        };
        use std::io::Write;
        writeln!(
            temp,
            "{kind} {} {} {}",
            self.rgb[0], self.rgb[1], self.rgb[2]
        )
        .map_err(|e| e.to_string())?;
        temp.persist(path).map_err(|e| e.error.to_string())?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_custom_rgb_and_rejects_invalid_channels() {
        assert_eq!(
            ThemeSettings::parse("custom 12 34 56").unwrap().theme,
            PaperTheme::Custom([12, 34, 56])
        );
        assert!(ThemeSettings::parse("custom 256 0 0").is_none());
        assert!(ThemeSettings::parse("unknown 0 0 0").is_none());
        assert!(ThemeSettings::parse("custom 1 2").is_none());
    }
    #[test]
    fn saves_custom_theme_and_replaces_previous_preferences() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fpdf/theme.txt");
        ThemeSettings {
            theme: PaperTheme::Custom([10, 20, 30]),
            rgb: [10, 20, 30],
        }
        .write_to(&path)
        .unwrap();
        let restored = ThemeSettings::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(restored.theme, PaperTheme::Custom([10, 20, 30]));
        ThemeSettings {
            theme: PaperTheme::Green,
            rgb: restored.rgb,
        }
        .write_to(&path)
        .unwrap();
        let restored = ThemeSettings::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(restored.theme, PaperTheme::Green);
        assert_eq!(restored.rgb, [10, 20, 30]);
    }
}
