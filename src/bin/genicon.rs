use std::fs;
use std::path::Path;
use macxtreamer::icon;

fn main() {
    let size = 1024u32;
    let icon = icon::generate_icon(size);
    let img = image::RgbaImage::from_raw(icon.width, icon.height, icon.rgba)
        .expect("invalid RGBA buffer");
    let out_dir = Path::new("assets");
    fs::create_dir_all(out_dir).unwrap();
    let out = out_dir.join("icon_1024.png");
    img.save(&out).expect("failed to save icon png");
    println!("Wrote {}", out.display());
}
