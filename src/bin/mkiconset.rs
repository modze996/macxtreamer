use std::fs;
use std::path::Path;
use macxtreamer::icon;

fn main() {
    // 1) Basissprite erzeugen (1024x1024)
    let base_sz = 1024u32;
    let icon_data = icon::generate_icon(base_sz);
    let img = image::RgbaImage::from_raw(icon_data.width, icon_data.height, icon_data.rgba)
        .expect("invalid RGBA buffer");

    let out_dir = Path::new("assets/macxtreamer.iconset");
    fs::create_dir_all(out_dir).expect("create iconset dir");

    // 2) Zielgrößen (px)
    let targets = [
        (16, "icon_16x16.png"),
        (32, "icon_16x16@2x.png"),
        (32, "icon_32x32.png"),
        (64, "icon_32x32@2x.png"),
        (128, "icon_128x128.png"),
        (256, "icon_128x128@2x.png"),
        (256, "icon_256x256.png"),
        (512, "icon_256x256@2x.png"),
        (512, "icon_512x512.png"),
        (1024, "icon_512x512@2x.png"),
    ];

    for (sz, name) in targets {        
        let resized = if sz == base_sz { img.clone() } else {
            image::imageops::resize(&img, sz, sz, image::imageops::FilterType::Lanczos3)
        };
        let path = out_dir.join(name);
        resized
            .save(&path)
            .unwrap_or_else(|e| panic!("failed to save {}: {}", path.display(), e));
    }

    println!("Iconset written to {}", out_dir.display());
}
