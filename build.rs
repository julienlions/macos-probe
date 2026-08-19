// jpegli is built by the CI step just before this; it tells us where.
fn main() {
    let lib = std::env::var("JPEGLI_LIB_DIR").expect("JPEGLI_LIB_DIR not set");
    let hwy = std::env::var("HWY_LIB_DIR").expect("HWY_LIB_DIR not set");
    println!("cargo:rustc-link-search=native={lib}");
    println!("cargo:rustc-link-search=native={hwy}");
    println!("cargo:rustc-link-lib=static=jpegli-static");
    println!("cargo:rustc-link-lib=static=hwy");
    // jpegli is C++; on non-MSVC targets the C++ runtime must be named.
    let t = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if t == "macos" {
        println!("cargo:rustc-link-lib=dylib=c++");
    } else if t == "linux" {
        println!("cargo:rustc-link-lib=dylib=stdc++");
    }
}
