use super::*;

#[test]
fn default_is_valid() {
    assert!(Settings::default().validate().is_ok());
    for p in Preset::ALL {
        assert!(Settings::with_preset(*p, &Settings::default()).validate().is_ok());
    }
}

#[test]
fn output_sizes() {
    let mut s = Settings::default();
    assert_eq!(s.output_size(1920, 1080), (1920, 1080));

    s.res_mode = ResMode::P720;
    assert_eq!(s.output_size(1920, 1080), (1280, 720));
    assert_eq!(s.output_size(640, 360), (640, 360));
    assert_eq!(s.output_size(1366, 768), (1280, 720));

    s.res_mode = ResMode::Percent;
    s.res_percent = 50.0;
    assert_eq!(s.output_size(1920, 1080), (960, 540));

    s.res_mode = ResMode::Custom;
    s.res_w = 1000;
    s.res_h = 1000;
    s.keep_aspect = true;
    assert_eq!(s.output_size(1920, 1080), (1000, 562));
    s.keep_aspect = false;
    assert_eq!(s.output_size(1920, 1080), (1000, 1000));
}

#[test]
fn bitrate() {
    let mut s = Settings::default();
    let b = s.bitrate_bps(1920, 1080);
    assert!((6_200_000..6_240_000).contains(&b));
    s.auto_bitrate = false;
    s.bitrate_kbps = 8000;
    assert_eq!(s.bitrate_bps(1920, 1080), 8_000_000);
}

#[test]
fn region_clamp() {
    let r = Region { x: 100, y: 50, w: 801, h: 601 };
    assert_eq!(r.clamp_to(1920, 1080), Some(Region { x: 100, y: 50, w: 800, h: 600 }));
    let r = Region { x: 1900, y: 0, w: 500, h: 500 };
    let c = r.clamp_to(1920, 1080).unwrap();
    assert!(c.x + c.w <= 1920);
    assert_eq!(Region { x: 0, y: 0, w: 8, h: 8 }.clamp_to(1920, 1080), None);
}

#[test]
fn conf_round_trip() {
    let mut a = Settings::with_preset(Preset::Ultra, &Settings::default());
    a.preset = Preset::Custom;
    a.qp_min = 12;
    a.qp_max = 38;
    a.threads = 4;
    a.auto_bitrate = false;
    a.bitrate_kbps = 12345;
    let text = a.to_conf("Microphone");
    let mut b = Settings::default();
    let audio = b.apply_conf(&text);
    assert_eq!(a, b);
    assert_eq!(audio.as_deref(), Some("Microphone"));
}
