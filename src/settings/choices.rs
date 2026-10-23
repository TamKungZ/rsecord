use std::fmt;

macro_rules! choice {
    ($name:ident { $($var:ident => $label:expr),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name { $($var),+ }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$var),+];
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(match self { $($name::$var => $label),+ })
            }
        }
    };
}

choice!(AppTheme {
    System => "Follow system",
    BrandDark => "rsecord Dark",
    BrandLight => "rsecord Light",
    Midnight => "Midnight (OLED)",
    Nord => "Nord",
    Dracula => "Dracula",
    Mocha => "Catppuccin Mocha",
    Latte => "Catppuccin Latte",
    TokyoNight => "Tokyo Night",
    Gruvbox => "Gruvbox Dark",
    SolarizedLight => "Solarized Light",
});

choice!(Preset {
    Low => "Low (small file)",
    Balanced => "Balanced",
    High => "High quality",
    Ultra => "Ultra (60 fps)",
    Custom => "Custom",
});

choice!(ResMode {
    Source => "Same as source",
    P2160 => "2160p (4K)",
    P1440 => "1440p",
    P1080 => "1080p",
    P720 => "720p",
    P480 => "480p",
    P360 => "360p",
    Percent => "Scale by percent",
    Custom => "Custom size",
});

impl ResMode {
    pub(super) fn target_height(self) -> Option<u32> {
        match self {
            ResMode::P2160 => Some(2160),
            ResMode::P1440 => Some(1440),
            ResMode::P1080 => Some(1080),
            ResMode::P720 => Some(720),
            ResMode::P480 => Some(480),
            ResMode::P360 => Some(360),
            _ => None,
        }
    }
}

choice!(RateMode {
    Bitrate => "Target bitrate",
    Quality => "Quality (QP range)",
});

choice!(Effort {
    Fast => "Fast (low CPU)",
    Balanced => "Balanced",
    Best => "Best quality (high CPU)",
});

choice!(Filter {
    Nearest => "Nearest (fastest)",
    Bilinear => "Bilinear",
    Bicubic => "Bicubic",
    Lanczos => "Lanczos (sharpest)",
});

choice!(Content {
    Screen => "Screen / UI",
    Motion => "Camera / motion",
});

pub(super) fn pick<T: Copy + fmt::Display>(all: &[T], s: &str) -> Option<T> {
    all.iter().copied().find(|v| v.to_string() == s)
}
