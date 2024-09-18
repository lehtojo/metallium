fn main() {
    println!("cargo:rustc-link-search=native=./low/");
    println!("cargo:rustc-link-lib=static=x64"); 
}
