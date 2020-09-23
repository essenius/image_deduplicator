use std::path::Path;
use std::fs::File;
use std::fs;
use std::io;
use sha2::{Sha256, Digest};
use std::error::Error;
use image::GenericImageView;

fn hash(file: &String) -> Result<String, io::Error> {
    let path = Path::new(file);

    let mut file = File::open(&path)?;
    let mut sha256 = Sha256::new();
    io::copy(&mut file, &mut sha256).expect("copy failed");
    Ok(format!("{:x}", sha256.finalize()))
}

fn main() -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir("c:\\data")?  {
        let entry = entry?;
        let path = entry.path();

        let metadata = fs::metadata(&path)?;
        if metadata.is_file() {
            let name = format!("{}",path.display());
            print!("Name: {}, size: {}", name, metadata.len());
            let img = match image::open(&name) {
                Ok(image_file) => image_file,
                Err(_error) => {
                    println!(": No image");
                    continue;
                }
            };
            println!(", dimensions {:?}, hash: {}", img.dimensions(), hash(&name).unwrap());
        }
    }
    Ok(())
}
