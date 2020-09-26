use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::fs::{self,File};
use sha2::{Sha256, Digest};
use std::error::Error;
//use image::GenericImageView;
use filetime::FileTime;


#[derive(Debug, Clone)]
struct ImageData {
    path: String,
    //dimensions: (u32, u32),
    size: u64,
    //hash: String,
}

fn hash(file: &String) -> Result<String, io::Error> {
    let path = Path::new(file);

    let mut file = File::open(&path)?;
    let mut sha256 = Sha256::new();
    io::copy(&mut file, &mut sha256).expect("copy failed");
    Ok(format!("{:x}", sha256.finalize()))
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    let path = PathBuf::from(&args[1]);
    if !&path.exists() {
        println!("Path '{}' does not exist", path.clone().into_os_string().into_string().unwrap());
    }

    let mut images : Vec<ImageData> = Vec::new();

    for entry in fs::read_dir(path)?  {
        let entry = entry?;
        let path = entry.path();

        let metadata = fs::metadata(&path)?;
        if metadata.is_file() {
            if  FileTime::from_last_modification_time(&metadata)== FileTime::zero() {
                let ctime = FileTime::from_creation_time(&metadata).unwrap();
                println!("Setting modified time to {}", &ctime);
                filetime::set_file_mtime(&path, ctime).unwrap();
            }
            let name = format!("{}",path.display());
/*             let dimensions = match image::open(&name) {
                Ok(image_file) => image_file.dimensions(),
                Err(error) => {
                    println!("Not an image: {}. Error: {}", &name, error);
                    (0, 0)
                }
            };  */
            images.push(ImageData{path: name.clone(), size: metadata.len()});
            print!(".");
        }
        io::stdout().flush().unwrap();
    }
    images.sort_by(|a, b| a.size.cmp(&b.size));
    let mut previous_image: ImageData  =  ImageData {path: String::new(), size: 0};
    for entry in &images {
        if previous_image.size == entry.size {
            let previous_hash = hash(&previous_image.path).unwrap();
            let current_hash = hash(&entry.path).unwrap();
            if previous_hash == current_hash {
                println!{"Found duplicate"};
                println!("  {:?}", previous_image);
                println!("  {:?}", entry);
                fs::rename(&entry.path, format!("{}.duplicate", &entry.path)).unwrap();    
            }
        }
        previous_image = Clone::clone(entry);
    }
    Ok(())
}
