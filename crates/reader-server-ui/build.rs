use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

fn collect(root: &Path, directory: &Path, output: &mut Vec<(String, PathBuf)>) {
    let mut entries: Vec<_> = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("cannot read UI directory {}: {error}", directory.display()))
        .map(|entry| entry.expect("valid UI directory entry"))
        .collect();
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, output);
        } else {
            let relative = path.strip_prefix(root).expect("asset under UI root");
            output.push((
                format!("/{}", relative.to_string_lossy().replace('\\', "/")),
                path,
            ));
        }
    }
}

fn content_type(path: &str) -> &'static str {
    if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".js") {
        "text/javascript; charset=utf-8"
    } else if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".json") {
        "application/json"
    } else {
        "application/octet-stream"
    }
}

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let root = manifest.join("../../web/dist");
    if !root.join("index.html").is_file() {
        panic!("web/dist is missing; run `cd web && npm run build` before compiling the embedded UI crate");
    }
    println!("cargo:rerun-if-changed={}", root.display());
    let mut assets = Vec::new();
    collect(&root, &root, &mut assets);
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("out dir"));
    let mut generated =
        fs::File::create(out_dir.join("assets.rs")).expect("create generated asset table");
    writeln!(generated, "pub static ASSETS: &[Asset] = &[").unwrap();
    for (index, (path, source)) in assets.iter().enumerate() {
        let copied = out_dir.join(format!("asset-{index}"));
        fs::copy(source, &copied).expect("copy UI asset to build output");
        writeln!(
            generated,
            "Asset {{ path: {:?}, content_type: {:?}, bytes: include_bytes!({:?}) }},",
            path,
            content_type(path),
            copied
        )
        .unwrap();
    }
    writeln!(generated, "];").unwrap();
}
