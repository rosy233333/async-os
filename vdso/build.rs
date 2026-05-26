use build_vdso::*;

fn main() {
    let mut config = BuildConfig::new(
        "../../vsched2",
        "vsched2",
    );
    // config.so_name = String::from("libvdsoexample");
    // config.api_lib_name = String::from("libvdsoexample");
    config.out_dir = String::from("../vdso_output");
    build_vdso(&config);
}
