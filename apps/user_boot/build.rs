fn main() {
    let pwd = std::env::current_dir().unwrap();
    let mut ancestors = pwd.ancestors();
    let uapps_dir = ancestors.nth(2).unwrap().join("user_apps");
    println!("cargo:rerun-if-changed={}", uapps_dir.display());
    println!("cargo:rerun-if-changed=./build.rs");
}
