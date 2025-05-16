use std::fs;
use std::io::Write;

fn main() {
    build_vdso();
    println!("cargo:rerun-if-changed=src/*");
    println!("cargo:rerun-if-changed=./cops.lds");
}

fn build_vdso() {
    const COPS_API_FILE_PATH: &str = "cops/src/api.rs";
    let cops_api_file_content = fs::read_to_string(COPS_API_FILE_PATH).unwrap();
    let re = regex::Regex::new(
        r#"#\[no_mangle\]\npub extern \"C\" fn ([a-zA-Z0-9_]?.*)(\([a-zA-Z0-9_:]?.*\)[->]?.*) \{"#,
    )
    .unwrap();
    // 获取共享调度器的 api
    let mut fns = vec![];
    for (_, [name, args]) in re
        .captures_iter(&cops_api_file_content)
        .map(|c| c.extract())
    {
        // println!("{}: {}", name, args);
        fns.push((name, args));
    }
    // vdso_vtable 数据结构定义
    let mut vdso_vtable_struct_str = "\nstruct VdsoVTable {\n".to_string();
    for (name, args) in fns.iter() {
        vdso_vtable_struct_str.push_str(&format!("    pub {}: Option<fn{}>,\n", name, args));
    }
    vdso_vtable_struct_str.push_str("}\n");
    // println!("vdso_vtable_str: {}", vdso_vtable_struct_str);

    // 定义静态的 VDSO_VTABLE
    let mut static_vdso_vtable_str =
        "\nstatic mut VDSO_VTABLE: VdsoVTable = VdsoVTable {\n".to_string();
    for (name, _) in fns.iter() {
        static_vdso_vtable_str.push_str(&format!("    {}: None,\n", name));
    }
    static_vdso_vtable_str.push_str("};\n");

    // 运行时初始化 vdso_table 的函数
    let mut fn_init_vdso_vtable_str = INIT_VDSO_VTABLE_STR.to_string();
    for (name, args) in fns.iter() {
        fn_init_vdso_vtable_str.push_str(&format!(
            r#"            if name == "{}" {{
                let fn_ptr = base + dynsym.value();
                log::debug!("{{}}: {{:x}}", name, fn_ptr);
                let f: fn{} = unsafe {{ core::mem::transmute(fn_ptr) }};
                VDSO_VTABLE.{} = Some(f);
            }}
"#,
            name, args, name
        ));
    }
    fn_init_vdso_vtable_str.push_str(
        r#"        }
    }
}
    "#,
    );
    // println!("fn_init_vdso_vtable_str: {}", fn_init_vdso_vtable_str);

    // 构建给内核和用户运行时使用的接口
    let mut apis = vec![];
    for (name, args) in fns.iter() {
        let re = regex::Regex::new(r#"\(([a-zA-Z0-9_:]?.*)\)"#).unwrap();
        let mut fn_args = String::new();
        for (_, [ident_ty]) in re.captures_iter(args).map(|c| c.extract()) {
            // println!("{}: {}", name, args);
            let ident_str: Vec<&str> = ident_ty
                .split(",")
                .map(|s| {
                    let idx = s.find(":");
                    if let Some(idx) = idx {
                        let ident = s[..idx].trim();
                        ident
                    } else {
                        ""
                    }
                })
                .collect();
            for ident in ident_str.iter() {
                if ident.len() > 0 {
                    fn_args.push_str(&format!("{}, ", ident));
                }
            }
            fn_args = fn_args.trim_end_matches(", ").to_string();
            // println!("{:?}", fn_args);
        }

        apis.push(format!(
            r#"
pub fn {}{} {{
    if let Some(f) = unsafe {{ VDSO_VTABLE.{} }} {{
        f({})
    }} else {{
        panic!("{} is not initialized")
    }}
}}
"#,
            name, args, name, fn_args, name
        ));
    }
    // println!("apis: {:?}", apis);

    // 生成最终的 api.rs 文件
    const API_FILE_PATH: &str = "src/api.rs";
    let mut api_file_content = fs::OpenOptions::new()
        .write(true)
        .open(API_FILE_PATH)
        .unwrap();
    api_file_content.write_all(VDSO_SECTION.as_bytes()).unwrap();

    api_file_content
        .write_all(vdso_vtable_struct_str.as_bytes())
        .unwrap();

    api_file_content
        .write_all(static_vdso_vtable_str.as_bytes())
        .unwrap();

    api_file_content
        .write_all(fn_init_vdso_vtable_str.as_bytes())
        .unwrap();

    for api in apis.iter() {
        api_file_content.write_all(api.as_bytes()).unwrap();
    }
}

const INIT_VDSO_VTABLE_STR: &str = r#"
pub unsafe fn init_vdso_vtable(base: u64, vdso_elf: &ElfFile) {
    if let Some(dyn_sym_table) = vdso_elf.find_section_by_name(".dynsym") {
        let dyn_sym_table = match dyn_sym_table.get_data(&vdso_elf) {
            Ok(xmas_elf::sections::SectionData::DynSymbolTable64(dyn_sym_table)) => dyn_sym_table,
            _ => panic!("Invalid data in .dynsym section"),
        };
        for dynsym in dyn_sym_table {
            let name = dynsym.get_name(&vdso_elf).unwrap();
"#;

const VDSO_SECTION: &str = r#"//! 这里的与 vDSO 相关的实现可以在 build 脚本中来自动化构建，而不是手动构建出来
use crate::id::TaskId;
use xmas_elf::symbol_table::Entry;
use xmas_elf::ElfFile;

extern "C" {
    fn vdso_sdata();
    fn vdso_edata();
    fn vdso_start();
    fn vdso_end();
}

pub fn get_vdso_base_end() -> (u64, u64, u64, u64) {
    (
        vdso_sdata as _,
        vdso_edata as _,
        vdso_start as _,
        vdso_end as _,
    )
}"#;
