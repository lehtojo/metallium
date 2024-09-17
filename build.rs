fn main() {
    println!("cargo:rustc-link-search=native=/home/lehtojo/Projects/kernel/low/");
    println!("cargo:rustc-link-lib=static=x64"); 
}
