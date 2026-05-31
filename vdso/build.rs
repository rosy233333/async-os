use build_vdso::*;

fn main() {
    let mut config = BuildConfig::new("../../vsched2", "vsched2");
    // config.so_name = String::from("libvdsoexample");
    // config.api_lib_name = String::from("libvdsoexample");
    config.out_dir = String::from("../vdso_output");
    config.toolchain = String::from("nightly-2026-05-26");
    config.features = vec![String::from("vdso_only")];
    build_vdso(&config);
}
