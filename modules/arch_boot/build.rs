const BUILTIN_PLATFORMS: &[&str] = &["riscv64-qemu-virt"];

const BUILTIN_PLATFORM_FAMILIES: &[&str] = &["riscv64-qemu-virt"];

fn make_cfg_values(str_list: &[&str]) -> String {
    str_list
        .iter()
        .map(|s| format!("{:?}", s))
        .collect::<Vec<_>>()
        .join(", ")
}

fn main() {
    let platform = axconfig::PLATFORM;

    println!("cargo:rustc-cfg=platform=\"{}\"", platform);
    println!("cargo:rustc-cfg=platform_family=\"{}\"", axconfig::FAMILY);

    println!(
        "cargo::rustc-check-cfg=cfg(platform, values({}))",
        make_cfg_values(BUILTIN_PLATFORMS)
    );
    println!(
        "cargo::rustc-check-cfg=cfg(platform_family, values({}))",
        make_cfg_values(BUILTIN_PLATFORM_FAMILIES)
    );
}
